package workspace

// TODO:
//   - AddressSpace interface
//   - RVA type
//   - VA type
//   - higher level maps api
//     - track allocations
//     - snapshot, revert, commit
//  - then, forward-emulate one instruction (via code hook) to get next insn

import (
	"bytes"
	"encoding/binary"
	"errors"
	"fmt"
	"github.com/bnagy/gapstone"
	uc "github.com/unicorn-engine/unicorn/bindings/go/unicorn"
	"io"
)

var PAGE_SIZE uint64 = 0x1000

func check(e error) {
	if e != nil {
		panic(e)
	}
}

func roundUp(i uint64, base uint64) uint64 {
	if i%base == 0x0 {
		return i
	} else {
		return i + base - (i % base)
	}
}

func roundUpToPage(i uint64) uint64 {
	return roundUp(i, PAGE_SIZE)
}

type Arch string
type Mode string

const ARCH_X86 Arch = "x86"
const MODE_32 Mode = "32"
const MODE_64 Mode = "64"

var GAPSTONE_ARCH_MAP = map[Arch]int{
	ARCH_X86: gapstone.CS_ARCH_X86,
}

var GAPSTONE_MODE_MAP = map[Mode]uint{
	MODE_32: gapstone.CS_MODE_32,
}

var InvalidArchError = errors.New("Invalid ARCH provided.")
var InvalidModeError = errors.New("Invalid MODE provided.")

type VA uint64
type RVA uint64

func (rva RVA) VA(baseAddress VA) VA {
	return VA(uint64(rva) + uint64(baseAddress))
}

type LoadedModule struct {
	Name        string
	BaseAddress VA
	EntryPoint  VA
}

func (m LoadedModule) VA(rva RVA) VA {
	return rva.VA(m.BaseAddress)
}

// note: relative to the module
func (m LoadedModule) MemRead(ws *Workspace, rva RVA, length uint64) ([]byte, error) {
	return ws.MemRead(m.VA(rva), length)
}

// note: relative to the module
func (m LoadedModule) MemReadPtr(ws *Workspace, rva RVA) (VA, error) {
	if ws.Mode == MODE_32 {
		var data uint32
		d, e := m.MemRead(ws, rva, 0x4)
		if e != nil {
			return 0, e
		}

		p := bytes.NewBuffer(d)
		binary.Read(p, binary.LittleEndian, &data)
		return VA(uint64(data)), nil
	} else if ws.Mode == MODE_64 {
		var data uint64
		d, e := m.MemRead(ws, rva, 0x8)
		if e != nil {
			return 0, e
		}

		p := bytes.NewBuffer(d)
		binary.Read(p, binary.LittleEndian, &data)
		return VA(uint64(data)), nil
	} else {
		return 0, InvalidModeError
	}
}

// note: relative to the module
func (m LoadedModule) MemWrite(ws *Workspace, rva RVA, data []byte) error {
	return ws.MemWrite(m.VA(rva), data)
}

type MemoryRegion struct {
	Address VA
	Length  uint64
	Name    string
}

type Workspace struct {
	// we cheat and use u as the address space
	u             uc.Unicorn
	Arch          Arch
	Mode          Mode
	loadedModules []*LoadedModule
	memoryRegions []MemoryRegion
}

func New(arch Arch, mode Mode) (*Workspace, error) {
	if arch != ARCH_X86 {
		return nil, InvalidArchError
	}
	if !(mode == MODE_32 || mode == MODE_64) {
		return nil, InvalidModeError
	}

	u, e := uc.NewUnicorn(uc.ARCH_X86, uc.MODE_32)
	if e != nil {
		return nil, e
	}

	return &Workspace{
		u:             u,
		Arch:          arch,
		Mode:          mode,
		loadedModules: make([]*LoadedModule, 1),
		memoryRegions: make([]MemoryRegion, 5),
	}, nil
}

func (ws *Workspace) MemRead(va VA, length uint64) ([]byte, error) {
	return ws.u.MemRead(uint64(va), length)
}

func (ws *Workspace) MemWrite(va VA, data []byte) error {
	return ws.u.MemWrite(uint64(va), data)
}

func (ws *Workspace) MemMap(va VA, length uint64, name string) error {
	e := ws.u.MemMap(uint64(va), length)
	if e != nil {
		return e
	}

	ws.memoryRegions = append(ws.memoryRegions, MemoryRegion{va, length, name})

	return nil
}

func (ws *Workspace) MemUnmap(va VA, length uint64) error {
	e := ws.u.MemUnmap(uint64(va), length)
	if e != nil {
		return e
	}

	// TODO: remove from map
	//ws.memoryRegions = append(ws.memoryRegions, MemoryRegion{va, length, name})

	return nil
}

func (ws *Workspace) AddLoadedModule(mod *LoadedModule) error {
	ws.loadedModules = append(ws.loadedModules, mod)
	return nil
}

func (ws *Workspace) getDisassembler() (*gapstone.Engine, error) {
	engine, e := gapstone.New(
		GAPSTONE_ARCH_MAP[ws.Arch],
		GAPSTONE_MODE_MAP[ws.Mode],
	)
	return &engine, e
}

func (ws *Workspace) disassembleBytes(data []byte, address VA, w io.Writer) error {
	// TODO: cache the engine on the Workspace?

	engine, e := ws.getDisassembler()
	check(e)
	defer engine.Close()

	insns, e := engine.Disasm([]byte(data), uint64(address), 0 /* all instructions */)
	check(e)

	w.Write([]byte(fmt.Sprintf("Disasm:\n")))
	for _, insn := range insns {
		w.Write([]byte(fmt.Sprintf("0x%x:\t%s\t\t%s\n", insn.Address, insn.Mnemonic, insn.OpStr)))
	}

	return nil
}

func (ws *Workspace) Disassemble(address VA, length uint64, w io.Writer) error {
	d, e := ws.MemRead(address, length)
	check(e)
	return ws.disassembleBytes(d, address, w)
}

var FailedToDisassembleInstruction = errors.New("Failed to disassemble an instruction")

func (ws *Workspace) DisassembleInstruction(address VA) (string, error) {
	engine, e := ws.getDisassembler()
	check(e)
	defer engine.Close()

	MAX_INSN_SIZE := 0x10
	d, e := ws.MemRead(address, uint64(MAX_INSN_SIZE))
	check(e)

	insns, e := engine.Disasm(d, uint64(address), 1)
	check(e)

	for _, insn := range insns {
		return fmt.Sprintf("0x%x: %s\t\t%s\n", insn.Address, insn.Mnemonic, insn.OpStr), nil
	}
	return "", FailedToDisassembleInstruction
}

func (ws *Workspace) GetInstructionLength(address VA) (uint64, error) {
	engine, e := ws.getDisassembler()
	check(e)
	defer engine.Close()

	MAX_INSN_SIZE := 0x10
	d, e := ws.MemRead(address, uint64(MAX_INSN_SIZE))
	check(e)

	insns, e := engine.Disasm(d, uint64(address), 1)
	check(e)

	for _, insn := range insns {
		// return the first one
		return uint64(insn.Size), nil
	}
	return 0, FailedToDisassembleInstruction
}

// TODO: make Emulator object and use that
func (ws *Workspace) SetStackPointer(address VA) error {
	// TODO: switch on arch
	return ws.u.RegWrite(uc.X86_REG_ESP, uint64(address))
}

func (ws *Workspace) GetStackPointer() (VA, error) {
	// TODO: switch on arch
	r, e := ws.u.RegRead(uc.X86_REG_ESP)
	if e != nil {
		return 0, e
	}
	return VA(r), e
}

func (ws *Workspace) Emulate(start VA, end VA) error {
	stackAddress := VA(0x69690000)
	stackSize := uint64(0x4000)
	e := ws.MemMap(VA(uint64(stackAddress)-(stackSize/2)), stackSize, "stack")
	check(e)

	defer func() {
		e := ws.MemUnmap(VA(uint64(stackAddress)-(stackSize/2)), stackSize)
		check(e)
	}()

	e = ws.SetStackPointer(stackAddress)
	check(e)

	esp, e := ws.GetStackPointer()
	check(e)
	fmt.Printf("esp: 0x%x\n", esp)

	ws.u.HookAdd(uc.HOOK_BLOCK, func(mu uc.Unicorn, addr uint64, size uint32) {
		//fmt.Printf("Block: 0x%x, 0x%x\n", addr, size)
	})

	ws.u.HookAdd(uc.HOOK_CODE, func(mu uc.Unicorn, addr uint64, size uint32) {
		insn, e := ws.DisassembleInstruction(VA(addr))
		check(e)
		fmt.Printf("%s", insn)
	})

	ws.u.HookAdd(uc.HOOK_MEM_READ|uc.HOOK_MEM_WRITE,
		func(mu uc.Unicorn, access int, addr uint64, size int, value int64) {
			if access == uc.MEM_WRITE {
				fmt.Printf("Mem write")
			} else {
				fmt.Printf("Mem read")
			}
			fmt.Printf(": @0x%x, 0x%x = 0x%x\n", addr, size, value)
		})

	invalid := uc.HOOK_MEM_READ_INVALID | uc.HOOK_MEM_WRITE_INVALID | uc.HOOK_MEM_FETCH_INVALID
	ws.u.HookAdd(invalid, func(mu uc.Unicorn, access int, addr uint64, size int, value int64) bool {
		switch access {
		case uc.MEM_WRITE_UNMAPPED | uc.MEM_WRITE_PROT:
			fmt.Printf("invalid write")
		case uc.MEM_READ_UNMAPPED | uc.MEM_READ_PROT:
			fmt.Printf("invalid read")
		case uc.MEM_FETCH_UNMAPPED | uc.MEM_FETCH_PROT:
			fmt.Printf("invalid fetch")
		default:
			fmt.Printf("unknown memory error")
		}
		fmt.Printf(": @0x%x, 0x%x = 0x%x\n", addr, size, value)
		return false
	})

	ws.u.HookAdd(uc.HOOK_INSN, func(mu uc.Unicorn) {
		rax, _ := mu.RegRead(uc.X86_REG_RAX)
		fmt.Printf("Syscall: %d\n", rax)
	}, uc.X86_INS_SYSCALL)

	return nil
}
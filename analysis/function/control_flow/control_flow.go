package control_flow_analysis

import (
	//	"github.com/Sirupsen/logrus"
	"github.com/bnagy/gapstone"
	AS "github.com/williballenthin/Lancelot/address_space"
	"github.com/williballenthin/Lancelot/artifacts"
	LD "github.com/williballenthin/Lancelot/disassembly/linear_disassembler"
	P "github.com/williballenthin/Lancelot/persistence"
	W "github.com/williballenthin/Lancelot/workspace"
)

func check(e error) {
	if e != nil {
		panic(e)
	}
}

type ControlFlowAnalysis struct {
	// referenced:
	ws *W.Workspace

	// owned:
}

func New(ws *W.Workspace) (*ControlFlowAnalysis, error) {
	return &ControlFlowAnalysis{
		ws: ws,
	}, nil
}

func (a *ControlFlowAnalysis) Close() error {
	return nil
}

/** ControlFlowAnalysis implements FunctionAnalysis interface **/
func (a *ControlFlowAnalysis) AnalyzeFunction(f *artifacts.Function) error {
	ld, e := LD.New(a.ws)
	check(e)

	cj, e := ld.RegisterJumpTraceHandler(func(
		insn gapstone.Instruction,
		from_bb AS.VA,
		target AS.VA,
		jtype P.JumpType) error {

		return a.ws.MakeCodeCrossReference(AS.VA(insn.Address), target, jtype)
	})
	check(e)
	defer ld.UnregisterJumpTraceHandler(cj)

	cb, e := ld.RegisterBBTraceHandler(func(start AS.VA, end AS.VA) error {
		return a.ws.MakeBasicBlock(start, end)
	})
	check(e)
	defer ld.UnregisterBBTraceHandler(cb)

	c, e := ld.RegisterInstructionTraceHandler(func(insn gapstone.Instruction) error {
		if disassembly.DoesInstructionHaveGroup(insn, gapstone.X86_GRP_CALL) {
			if insn.X86.Operands[0].Type == gapstone.X86_OP_IMM {
				// assume we have: call 0x401000
				targetva := AS.VA(insn.X86.Operands[0].Imm)
				a.ws.MakeFunction(targetva)
			}
		}
		return nil
	})
	check(e)
	defer ld.UnregisterInstructionTraceHandler(c)

	e = ld.ExploreFunction(a.ws, f.Start)
	check(e)

	return nil
}

func (a *ControlFlowAnalysis) Priority() uint {
	return 50
}

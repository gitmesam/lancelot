rule __EH_prolog3 : flirt
{
    strings:
        $__EH_prolog3_GS_align = {518B4C240C895C240C8D5C240C508D442408F7D923C18D60F88B43F08904248B}
        $__EH_prolog3_align = {518B4C240C895C240C8D5C240C508D442408F7D923C18D60F88B43F08904248B}
        $__EH_prolog3_catch_GS_align = {518B4C240C895C240C8D5C240C508D442408F7D923C18D60F88B43F08904248B }
        $__EH_prolog3_catch_align = {518B4C240C895C240C8D5C240C508D442408F7D923C18D60F88B43F08904248B }

    condition:
        any of them
}


module tb;
    logic a, b;
    int result;
    function automatic int select_branch(input logic aa, bb, cc, dd);
        if (aa &&& bb)
            if (cc &&& dd) return 11;
            else return 12;
        else if (cc &&& dd) return 13;
        return 14;
    endfunction
    initial begin
        if (select_branch(1, 1, 1, 1) != 11) $fatal(1, "nested then");
        if (select_branch(1, 1, 0, 1) != 12) $fatal(1, "dangling else");
        if (select_branch(0, 1, 1, 1) != 13) $fatal(1, "outer else");
        if (select_branch(0, 0, 0, 0) != 14) $fatal(1, "fallthrough return");
        result = 7; a = 0; b = 1;
        if (a &&& b) result = 99;
        if (result != 7) $fatal(1, "missing else");
        a = 1;
        if (a &&& b); else $fatal(1, "empty true statement");
        if (a &&& b) result = 9; else;
        if (result != 9) $fatal(1, "empty false statement");
        $display("branch_roles=pass");
        $finish(0);
    end
endmodule

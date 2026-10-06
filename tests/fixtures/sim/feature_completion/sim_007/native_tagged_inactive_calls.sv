// SIM-007-A03: inactive members of tagged unions in subroutine storage report
// at run time and store nothing, like module storage; an unknown conditional
// over differing tags leaves no member active.
typedef union tagged { int I; string S; void None; } val_t;

module tb;
    val_t r;

    function automatic string peek(val_t v);
        v.S = "w";
        return v.S;
    endfunction

    function automatic string bind_s(val_t v);
        if (v matches tagged S .s) return s;
        return "-";
    endfunction

    function automatic val_t pick(logic c, val_t a, val_t b);
        return c ? a : b;
    endfunction

    initial begin
        $display("[%s]", peek(tagged I 1));
        $display("[%s] [%s]", bind_s(tagged S "b"), bind_s(tagged None));
        r = pick(1'bx, tagged I 2, tagged S "y");
        $display("[%s]", r.S);
        $finish(0);
    end
endmodule

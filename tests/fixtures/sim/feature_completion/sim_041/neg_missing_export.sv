// SIM-041 negative: errors.c calls err_tick and err_kill, which this design
// does not export, so the model link names them (SV 35.7).
module q;
    import "DPI-C" context function int err_scope(input int n);
endmodule

module tb;
    export "DPI-C" function err_inc;
    export "DPI-C" function err_text;

    function int err_inc(input int n);
        return n + 1;
    endfunction

    function string err_text(input string s);
        return {s, "!"};
    endfunction

    q uq();
    int r = 0;

    initial begin
        $display("start");
        r = uq.err_scope(1);
        $display("not reached r=%0d", r);
        $finish;
    end
endmodule

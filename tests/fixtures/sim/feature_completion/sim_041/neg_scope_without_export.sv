// SIM-041 negative: a context import whose scope does not export the called
// subroutine, and no svSetScope (SV 35.5.3).
module q;
    import "DPI-C" context function int err_scope(input int n);
endmodule

module tb;
    export "DPI-C" function err_inc;
    export "DPI-C" function err_text;
    export "DPI-C" task err_tick;
    export "DPI-C" function err_kill;

    function int err_inc(input int n);
        return n + 1;
    endfunction

    function string err_text(input string s);
        return {s, "!"};
    endfunction

    task err_tick();
    endtask

    function void err_kill();
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

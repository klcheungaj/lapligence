// SIM-041 negative: a disabled imported function returns without svAckDisabledState (SV 35.9 c).
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
        disable body;
    endfunction

    import "DPI-C" context function int err_no_ack();

    int r = 0;

    initial begin
        $display("start");
        begin : body
            r = err_no_ack();
        end
        $display("not reached r=%0d", r);
        $finish;
    end
endmodule

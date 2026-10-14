// SIM-041 negative: an imported function calls an exported task (SV 35.8).
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

    import "DPI-C" context function int err_task_from_function();

    int r = 0;

    initial begin
        $display("start");
        begin : body
            r = err_task_from_function();
        end
        $display("not reached r=%0d", r);
        $finish;
    end
endmodule

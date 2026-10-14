// SIM-041 A03: the DPI disable protocol in zero-time chains (SV 35.9). An
// exported task or function that disables the block running the import ends
// that block once the import has acknowledged the disable; an exported task
// that disables itself returns 0 and leaves the import enabled.
module tb;
    import "DPI-C" context task dt_outer(input int n, output int o);
    import "DPI-C" context task dt_self_outer(output int o);
    import "DPI-C" context function int df_outer(input int n);

    export "DPI-C" task dt_kill;
    export "DPI-C" task dt_self;
    export "DPI-C" function df_kill;

    int seen = 0;
    int out = 0;
    int r = 0;

    task dt_kill(input int n);
        seen += n;
        disable outer_task;
    endtask

    task dt_self();
        seen += 100;
        disable dt_self;
        seen += 1000;
    endtask

    function int df_kill();
        disable outer_function;
        return 5;
    endfunction

    initial begin
        out = 11;
        begin : outer_task
            dt_outer(3, out);
            $display("outer_task not reached");
        end
        $display("after task disable seen=%0d out=%0d", seen, out);
        dt_self_outer(out);
        $display("after self disable seen=%0d out=%0d", seen, out);
        r = 1;
        begin : outer_function
            r = df_outer(4);
            $display("outer_function not reached");
        end
        $display("after function disable r=%0d", r);
        $finish;
    end
endmodule

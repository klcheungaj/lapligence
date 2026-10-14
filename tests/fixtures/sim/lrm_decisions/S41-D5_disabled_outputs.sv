// Decision S41-D5: when an imported task or function returns because a
// disable cancelled it, nothing is copied back: its outputs and inouts keep
// the values they had before the call and a function result is not
// assigned. An exported task that disabled the caller's block returns 1.
//
// IEEE 1800-2009 35.9 (SystemVerilog-1800-2009.txt L56269-56275):
//   "a) When an exported task returns due to a disable, it shall return a
//   value of 1. Otherwise, it shall return 0. b) When an imported task
//   returns due to a disable, it shall return a value of 1. [...] c) Before
//   an imported function returns due to a disable, it shall call the API
//   function svAckDisabledState()."
// 35.9 (L56313-56318): "When a DPI imported subroutine returns due to a
//   disable, the values of its output and inout parameters are undefined.
//   [...] SystemVerilog simulators are not obligated to propagate any such
//   values to the calling SystemVerilog code if a disable is in effect."
// 9.6.2 (L12400-12402) leaves a function's disable of a block that called it
// undefined; 35.9 c) is the DPI protocol for exactly that case and is
// followed here. Not propagating is an llg choice the text permits.
//
// Build S41-D5_disabled_outputs.c into a shared library and load it with the
// simulator's DPI library option.
module tb;
    import "DPI-C" context task d5_task(output int o);
    import "DPI-C" context function int d5_func(output int o);
    export "DPI-C" task d5_kill_task;
    export "DPI-C" function d5_kill_func;

    int o = 1;
    int r = 2;

    task d5_kill_task();
        disable blk_t;
    endtask

    function void d5_kill_func();
        disable blk_f;
    endfunction

    initial begin
        begin : blk_t
            d5_task(o);
        end
        $display("task o=%0d", o);
        begin : blk_f
            r = d5_func(o);
        end
        $display("func o=%0d r=%0d", o, r);
        $finish;
    end
endmodule

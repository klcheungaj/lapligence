// IEEE 1364-2001 §3.10 / IEEE 1800-2009 §7.6: extent-independent source shape.
`ifndef RTL002_COUNT
`define RTL002_COUNT 16777216
`endif
module tb;
    reg [7:0] source [0:`RTL002_COUNT-1], target [0:`RTL002_COUNT-1];
    initial begin
        source[0] = 8'h12;
        source[`RTL002_COUNT-1] = 8'h34;
        target = source;
        if (target[0] !== 8'h12 || target[`RTL002_COUNT-1] !== 8'h34 || target[1] !== 8'bx) $fatal;
        $display("PASS rtl002 source shape");
        $finish(0);
    end
endmodule

// IEEE 1364-2001 §3.10: whole memories are not general value operands.
module tb;
    reg a[0:16777215], b[0:16777215];
    initial begin a = b; $finish(0); end
endmodule

// IEEE 1800-2009 10.3.2: this driver changes its own operand on every
// evaluation, so time cannot advance; the per-process zero-time step limit
// reports it.
module tb;
    bit [3:0] count;
    bit go = 0;
    assign count = go ? count + 4'd1 : 4'd0;
    initial begin
        #1 go = 1;
        #1 $finish;
    end
endmodule

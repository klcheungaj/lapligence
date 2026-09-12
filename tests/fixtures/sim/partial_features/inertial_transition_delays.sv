// IEEE 1364-2001 sections 6.1.3 and 7.14; IEEE 1800-2009 sections 10.3 and 28.16.
`timescale 1ns/1ps
module tb;
    reg source, enable;
    wire scalar, multi0, multi1;
    wire [3:0] gate_out;
    wire [3:0] vector, selected;
    assign #(2, 4, 6) scalar = source;
    assign #(2, 4, 6) vector = {4{source}};
    assign #(2, 4, 6) selected[1:0] = {2{source}};
    buf #(2, 4) multi_output(multi0, multi1, source);
    bufif1 #(2, 4, 6) gate_driver0(gate_out[0], source, enable);
    bufif1 #(2, 4, 6) gate_driver1(gate_out[1], source, enable);
    initial begin
        source = 0;
        enable = 1;
        #6 $display("t6 %b %b%b %b %b %b", scalar, multi1, multi0, gate_out, vector, selected);
        source = 1;
        #3 $display("t9 %b %b%b %b %b %b", scalar, multi1, multi0, gate_out, vector, selected);
        source = 1'bx;
        #3 $display("t12 %b %b%b %b %b %b", scalar, multi1, multi0, gate_out, vector, selected);
        source = 1'bz;
        #7 $display("t19 %b %b%b %b %b %b", scalar, multi1, multi0, gate_out, vector, selected);
        $finish(0);
    end
endmodule

// A concatenated actual has an expression but no single high declaration.
`timescale 1ns/1ns
module concat_child(inout wand [3:0] p, input wire [3:0] d);
    assign p = d;
endmodule
module tb;
    wire [1:0] upper;
    wire [0:1] lower;
    reg [3:0] parent_data, child_data;
    concat_child u({upper, lower}, child_data);
    assign upper = parent_data[3:2];
    assign lower = parent_data[1:0];
    initial begin
        parent_data = 4'ha; child_data = 4'hc;
        #1 $display("concat=%b/%b/%b", upper, lower, u.p);
        parent_data = 4'hf; child_data = 4'h5;
        #1 $display("changed=%b/%b/%b", upper, lower, u.p);
        parent_data = 4'hc; child_data = 4'hz;
        #1 $display("parent_only=%b/%b/%b", upper, lower, u.p);
        parent_data = 4'hz;
        #1 $display("float=%b/%b/%b", upper, lower, u.p);
        $finish(0);
    end
endmodule

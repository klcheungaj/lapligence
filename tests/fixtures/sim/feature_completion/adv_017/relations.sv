// llg-test-fixture: ADV-017 assertion relations and sequence and property
// objects (IEEE 1800-2009 37.45-37.51, 39.3.2, 39.4.2). relations.c
// ($relations) walks every assertion's clocking event, disable condition,
// property, condition and action statements, the sequence and property
// instances and declarations they reach, places failure callbacks that
// print failExpr and instance callbacks. Expected output: relations.out
// (derivation in readme.md).
`timescale 1ns/1ns
module leaf(input logic clk, input logic a);
    default clocking cb @(posedge clk); endclocking
    default disable iff (!a);
    l_def: assert property (a);
endmodule

module tb;
    logic clk = 0, a = 1, b = 1, rst = 0;
    always #5 clk = ~clk;

    sequence s1(x, int n = 2);
        x ##n b;
    endsequence
    property p1(x, local input logic y);
        @(posedge clk) disable iff (rst) s1(x) |-> y;
    endproperty
    property p2(sequence s);
        @(posedge clk) s;
    endproperty

    a_spec: assert property (@(posedge clk) disable iff (rst) a |=> b)
        $display("%0t a_spec pass", $time);
    else
        $display("%0t a_spec fail", $time);
    a_inst: assert property (p1(a, b));
    c_seq: cover sequence (@(posedge clk) s1(a, 1));
    a_nest: assert property (p2(s1(a)));
    a_bool: assume property (@(posedge clk iff a) b);
    always @(negedge clk) begin
        a_proc: assert property (a ##1 b);
    end
    always @(posedge clk) begin
        i1: assert (a == b) else $display("%0t i1 fail", $time);
        i2: cover (a) $display("%0t i2 covered", $time);
    end
    leaf u_leaf(clk, a);

    initial begin
        $relations;
        #12 b = 1'b0;
        #10 b = 1'b1;
        #10 $finish;
    end
endmodule

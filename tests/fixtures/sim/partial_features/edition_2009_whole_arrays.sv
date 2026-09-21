// llg-test-fixture: SYN-019 SystemVerilog-2009 whole fixed-unpacked values
// IEEE 1800-2009 §§7.4.2, 7.6, and 11.4.11: unpacked array ports, whole-array
// assignment/equality, and array-valued conditional expressions.
module array_child(
    input logic [7:0] in_values [0:1],
    output logic [7:0] out_values [0:1]
);
    always_comb out_values = in_values;
endmodule

module tb;
    logic [7:0] a [0:1];
    logic [7:0] b [0:1];
    logic [7:0] c [0:1];
    logic select;
    array_child u(a, b);

    initial begin
        a[0] = 8'h12;
        a[1] = 8'h34;
        c[0] = 8'h56;
        c[1] = 8'h78;
        select = 1'b1;
        #0;
        b = select ? a : c;
        $display("whole=%h %h equal=%b", b[0], b[1], b == a);
        $finish;
    end
endmodule

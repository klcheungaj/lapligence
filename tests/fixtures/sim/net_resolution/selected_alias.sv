// llg-test-fixture: tests/fixtures/sim/net_resolution/selected_alias.sv
// LRM: IEEE 1800-2009 10.11.
module tb;
    wire [3:0] a;
    wire [7:0] b;
    alias a = b[3:0];
    assign a = 4'b1010;

    initial begin
        #1 $display("CHECK: selected=%b/%b", a, b);
        force b = 8'b11001100;
        #1 $display("CHECK: selected_force=%b/%b", a, b);
        release b;
        #1 $display("CHECK: selected_release=%b/%b", a, b);
        force a[3:2] = 2'b01;
        #1 $display("CHECK: selected_part_force=%b/%b", a, b);
        release a[3:2];
        #1 $display("CHECK: selected_part_release=%b/%b", a, b);
        force a[1] = 1'b0;
        #1 $display("CHECK: selected_bit_force=%b/%b", a, b);
        release a[1];
        #1 $display("CHECK: selected_bit_release=%b/%b", a, b);
        $finish(0);
    end
endmodule

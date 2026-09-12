// llg-test-fixture: tests/fixtures/sim/net_resolution/concat_alias.sv
// LRM: IEEE 1800-2009 10.11.
module tb;
    wire [1:0] a, b, c, d;
    alias {a, b} = {c, d};
    assign a = 2'b10;
    assign b = 2'b01;

    initial begin
        #1 $display("CHECK: concat=%b%b/%b%b", a, b, c, d);
        $finish(0);
    end
endmodule

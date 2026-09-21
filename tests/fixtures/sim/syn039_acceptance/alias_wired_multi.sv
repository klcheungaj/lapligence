// llg-test-fixture: SYN-039 aliased wired multi-instance network.
// IEEE 1800-2009 §§6.5, 6.6, 10.3, 10.11. Two instances contribute drivers
// through a true alias. The wired-AND resolver and both public net names keep
// the same resolved identity as enables change.
module wired_source #(
    parameter bit VALUE = 1'b0
) (
    input logic enable,
    inout wand bus
);
    assign bus = enable ? VALUE : 1'bz;
endmodule

module tb;
    wand network;
    wand mirror;
    logic enable_zero;
    logic enable_one;

    alias network = mirror;
    wired_source #(.VALUE(1'b0)) zero(.enable(enable_zero), .bus(network));
    wired_source #(.VALUE(1'b1)) one(.enable(enable_one), .bus(mirror));

    initial begin
        enable_zero = 1'b0;
        enable_one = 1'b0;
        #1 $display("none=%b/%b", network, mirror);
        enable_zero = 1'b1;
        #1 $display("zero=%b/%b", network, mirror);
        enable_zero = 1'b0;
        enable_one = 1'b1;
        #1 $display("one=%b/%b", network, mirror);
        enable_zero = 1'b1;
        #1 $display("both=%b/%b", network, mirror);
        $finish(0);
    end
endmodule

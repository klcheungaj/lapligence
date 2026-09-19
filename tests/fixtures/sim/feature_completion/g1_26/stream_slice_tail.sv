// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_26/stream_slice_tail.sv
// IEEE 1800-2009 11.4.14: a stream whose total width is not divisible by the
// slice size keeps the short left-most block in place instead of padding or
// truncating it, in both directions of assignment.
module tb;
    logic [7:0] lanes [0:2];
    logic [23:0] result;

    initial begin
        lanes[0] = 8'ha1;
        lanes[1] = 8'hb2;
        lanes[2] = 8'hc3;
        result = {<<5{lanes}};
        $display("rhs %h", result);

        lanes[0] = 8'h00;
        lanes[1] = 8'h00;
        lanes[2] = 8'h00;
        {<<5{lanes}} = 24'h1d_98_3a;
        $display("lhs %h %h %h", lanes[0], lanes[1], lanes[2]);
        $finish(0);
    end
endmodule

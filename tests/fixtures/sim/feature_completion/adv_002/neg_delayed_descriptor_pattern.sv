// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/neg_delayed_descriptor_pattern.sv
// IEEE 1800-2009 10.3.3, 10.9. A pattern member that is a 65,537-cell
// variable row has no packed driver value to delay; llg rejects the delayed
// driver with a diagnostic instead of dropping the delay.
module tb;
    typedef logic [7:0] row_t[0:65536];
    row_t d0, d1;
    row_t two[0:1];
    assign #2 '{d0, d1} = two;
    initial begin
        two[0][0] = 8'h03;
        #3 $display("%h %h", d0[0], d1[0]);
        $finish(0);
    end
endmodule

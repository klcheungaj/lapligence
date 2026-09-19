// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_26/bitstream_size_mismatch.sv
// IEEE 1800-2009 6.24.3: a bit-stream cast whose source width is not a
// multiple of the destination element width must fail instead of truncating,
// padding or inventing payload data.
module tb;
    typedef logic [3:0] lane_t;
    typedef lane_t lane3_t [0:2];
    typedef logic [7:0] word_t;
    typedef word_t word_dynamic_t[];
    lane3_t source;
    word_dynamic_t dst;

    initial begin
        source = '{4'ha, 4'hb, 4'hc};
        dst = word_dynamic_t'(source);
        $display("dst %0d", $size(dst));
        $finish(0);
    end
endmodule

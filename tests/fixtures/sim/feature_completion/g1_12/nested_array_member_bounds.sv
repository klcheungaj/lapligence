// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_12/nested_array_member_bounds.sv
// IEEE 1800-2009 7.4.6 and 11.5.3: an out-of-range or unknown fixed-array
// element index makes a composed packed selection read X and must not touch
// host memory; an in-range selection still observes the element.
module tb;
    logic [3:0][7:0] packed_elem [0:1];
    integer i;

    initial begin
        packed_elem[0] = 32'h1122_3344;
        packed_elem[1] = 32'haabb_ccdd;

        i = -1;
        $display("low %h %h", packed_elem[i][1], packed_elem[i][2][7:4]);
        i = 2;
        $display("high %h %h", packed_elem[i][1], packed_elem[i][3][0]);
        i = 32'bxxxxxxxx;
        $display("unknown %h %h", packed_elem[i][1], packed_elem[i][3 -: 2]);

        i = 1;
        $display(
            "read %h %h %h",
            packed_elem[i][0],
            packed_elem[i][2][7:4],
            packed_elem[i][3][0]
        );
        $finish(0);
    end
endmodule

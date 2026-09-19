// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_12/nested_array_member_select.sv
// IEEE 1800-2009 7.4, 10.10 and 11.5: a variable fixed-array index followed
// by constant packed dimension and part/bit selects writes only the selected
// bits of that element. Every result is observed through the whole element so
// the test does not depend on the separate select-read path.
module tb;
    logic [3:0][7:0] packed_elem [0:1];
    logic [15:0] words [0:3];
    int i;

    initial begin
        packed_elem[0] = 32'hde_ad_be_ef;
        packed_elem[1] = 32'h11_22_33_44;
        words[0] = 16'h0000;
        words[1] = 16'h0000;
        i = 1;

        // A whole packed dimension of the element: byte lane 1 is 0x33.
        packed_elem[i][1] = 8'haa;
        $display("whole %h", packed_elem[1]);

        // A part select within the selected dimension (byte lane 2 high nibble).
        packed_elem[i][2][7:4] = 4'hb;
        $display("part %h", packed_elem[1]);

        // A bit select within the selected dimension (byte lane 3 bit 0).
        packed_elem[i][3][0] = 1'b0;
        $display("bit %h", packed_elem[1]);

        // An indexed part select within the selected dimension (lane 0 bits 3:2).
        packed_elem[i][0][3 -: 2] = 2'b11;
        $display("indexed %h", packed_elem[1]);

        // The non-selected element and the simple fixed-array part select are
        // unchanged by the composed writes.
        words[2] = 16'h0000;
        words[2][11:8] = 4'hc;
        $display("other %h %h %h", packed_elem[0], packed_elem[1], words[2]);
        $finish(0);
    end
endmodule

// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_12/nested_array_member_read.sv
// IEEE 1800-2009 7.4, 10.10 and 11.5: a variable fixed-array index followed
// by constant packed dimension, part, bit and indexed-part selects reads the
// absolute slice those selectors name, in both declared descending and
// ascending packed dimensions.
module tb;
    logic [3:0][7:0] packed_elem [0:1];
    logic [0:3][7:0] asc_elem [0:1];
    logic [15:0] words [0:3];
    int i;

    initial begin
        packed_elem[0] = 32'hde_ad_be_ef;
        packed_elem[1] = 32'h11_22_33_44;
        asc_elem[1] = 32'h11_22_33_44;
        words[2] = 16'h0c00;
        i = 1;

        // Whole packed lanes of the selected element: 44 33 22 11.
        $display(
            "lane %h %h %h %h",
            packed_elem[i][0],
            packed_elem[i][1],
            packed_elem[i][2],
            packed_elem[i][3]
        );
        // Part selects within lane 2 (0x22): both nibbles are 2.
        $display("part %h %h", packed_elem[i][2][7:4], packed_elem[i][2][3:0]);
        // Bit selects within lane 3 (0x11): bit 0 and bit 4 are both 1.
        $display("bit %h %h", packed_elem[i][3][0], packed_elem[i][3][4]);
        // Indexed selects within lane 0 (0x44 = 0100_0100): 4, 4 and 1.
        $display(
            "indexed %h %h %h",
            packed_elem[i][0][7 -: 4],
            packed_elem[i][0][3 -: 4],
            packed_elem[i][0][2 +: 3]
        );
        // Ascending declaring range [0:3] keeps element 0 most significant.
        $display(
            "asc %h %h %h %h",
            asc_elem[i][0],
            asc_elem[i][1],
            asc_elem[i][2],
            asc_elem[i][3]
        );
        // The non-selected element and a plain fixed-array part select.
        $display("other %h %h", packed_elem[0], words[2][11:8]);
        $finish(0);
    end
endmodule

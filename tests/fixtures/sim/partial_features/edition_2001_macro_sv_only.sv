// llg-test-fixture: SYN-019 macro expansion remains under edition policy
// IEEE 1800-2009 §22.5: macro replacement does not change the selected edition.
`define SV_TYPE logic
module tb;
    `SV_TYPE value;
    initial begin
        value = 1'b0;
        $finish;
    end
endmodule

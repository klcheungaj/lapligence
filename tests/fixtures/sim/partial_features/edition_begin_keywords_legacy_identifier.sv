// llg-test-fixture: SYN-019 begin_keywords lexical compatibility
// IEEE 1800-2009 §22.14: an old keyword table makes `logic` an identifier;
// the selected global edition remains SystemVerilog-2009.
`begin_keywords "1364-2001"
module tb;
    reg logic;
    initial begin
        logic = 1'b1;
        $display("legacy_name=%b", logic);
        $finish;
    end
endmodule
`end_keywords

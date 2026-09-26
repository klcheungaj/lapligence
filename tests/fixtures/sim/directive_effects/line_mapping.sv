// IEEE 1800-2009 sections 22.12–22.13 (`line itself is also in
// IEEE 1364-2001 section 19.7):
// line state reaches the predefined file and line macros used by the
// executable design.
`line 123 "syn017_mapped.sv" 0
module tb;
  initial begin
    $display("file=%s line=%0d", `__FILE__, `__LINE__);
    $finish;
  end
endmodule

// Allocation failure matrix: packed values of every storage class (one
// word, two words, heap words), their arithmetic, formatting and conversion.
`timescale 1ns / 1ns
module tb;
  logic [63:0] narrow;
  logic [127:0] pair;
  logic [999:0] wide, other;
  logic signed [299:0] signed_wide;
  bit [4095:0] big;
  logic [399999:0] huge, product;
  real r;
  string digits;
  logic [199:0] memory[8];

  function automatic logic [999:0] mix(input logic [999:0] a, input logic [999:0] b);
    return (a ^ {b[499:0], b[999:500]}) + (a >> 13) - (b << 7);
  endfunction

  initial begin
    narrow = 64'hfedc_ba98_7654_3210;
    pair = {narrow, ~narrow};
    wide = '1;
    other = {8{pair[124:0]}};
    wide = mix(wide, other);
    $display("mix %h", wide[999:936]);
    wide = wide * other;
    $display("mul %h", wide[63:0]);
    wide = wide / (other | 1000'd3);
    $display("div %0d", wide[31:0]);
    wide[517] = 1'bx;
    $display("x %b", wide[519:515]);
    signed_wide = -300'sd123456789012345678901234567890;
    $display("dec %0d", signed_wide);
    signed_wide = signed_wide >>> 40;
    $display("sra %h", signed_wide[299:268]);
    big = 4096'd1 << 4000;
    big = big + big;
    $display("big %0d", $clog2(big));
    huge = '0;
    huge[399999] = 1'b1;
    huge[200003] = 1'b1;
    huge[199990] = 1'b1;
    huge[0] = 1'b1;
    product = 400000'(huge[199999:0]) * 400000'(huge[399999:200000]);
    huge = product / (huge[399999:200000] | 400000'd1);
    $display("huge %0d %0d", $countones(huge), $countones(product));
    digits = $sformatf("%0d", product[20000:0] | huge[30000:10000]);
    $display("digits %0d", digits.len());
    r = $itor(narrow[15:0]) * 1.5;
    narrow = 64'(unsigned'($rtoi(r))) + $realtobits(r);
    $display("real %h %0.3f", narrow, r);
    foreach (memory[i]) memory[i] = {i[2:0], 197'd0} | 200'(narrow) << (i * 9);
    $display("mem %h", memory[7][199:136] ^ memory[3][127:64]);
    $display("str %s|%10d|%o", string'(32'h41424344 + 32'(narrow[1:0])), memory[1][40:0], pair[20:0]);
    $display("done");
    $finish;
  end
endmodule

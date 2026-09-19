// R03: read and write oracles use whole parent words, not the same select mapper.
module tb;
  logic [1:0][0:7] ascending [0:0];
  logic [1:0][7:0] descending [0:0];
  logic [4:3][-2:5] negative_base [0:0];
  logic [4:3][12:5] positive_base [0:0];
  int index;
  initial begin
    ascending[0] = '0;
    ascending[0][1][2 +: 3] = 3'b111;
    if (ascending[0] !== 16'h3800) $fatal(1, "ascending constant +:");
    ascending[0] = '0;
    index = 4;
    ascending[0][1][index -: 3] = 3'b101;
    if (ascending[0] !== 16'h2800) $fatal(1, "ascending runtime -:");
    index = 2;
    if (ascending[0][1][index +: 3] !== 3'b101) $fatal(1, "ascending runtime read");
    ascending[0] = '0;
    ascending[0][1][2:4] = 3'b110;
    if (ascending[0] !== 16'h3000) $fatal(1, "ascending ordinary part");
    descending[0] = '0;
    descending[0][1][2 +: 3] = 3'b101;
    if (descending[0] !== 16'h1400) $fatal(1, "descending +:");
    descending[0] = '0;
    index = 4;
    descending[0][1][index -: 3] = 3'b110;
    if (descending[0] !== 16'h1800) $fatal(1, "descending -:");
    negative_base[0] = '0;
    index = -1;
    negative_base[0][4][index +: 3] = 3'b101;
    if (negative_base[0] !== 16'h5000) $fatal(1, "negative ascending +:");
    index = 1;
    if (negative_base[0][4][index -: 3] !== 3'b101) $fatal(1, "negative ascending -:");
    positive_base[0] = '0;
    index = 9;
    positive_base[0][4][index -: 3] = 3'b101;
    if (positive_base[0] !== 16'h1400) $fatal(1, "nonzero descending -:");
    index = 7;
    if (positive_base[0][4][index +: 3] !== 3'b101) $fatal(1, "nonzero descending +:");
    descending[0] = '0;
    descending[0]['1]['1] = 1'b1;
    if (descending[0] !== 16'h0200) $fatal(1, "self-determined fill index");
    $display("packed directions passed");
    $finish(0);
  end
endmodule

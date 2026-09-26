// IEEE 1800-2009 7.12.2: reverse permutes immediate unpacked elements.
`timescale 1ns/1ns
module tb;
  typedef struct { logic [7:0] tag; bit [3:0] payload; } record_t;

  logic [6:0] cube [0:1][1:0][2:0];
  record_t records [0:1][0:2];
  logic [6:0] observed;
  logic [7:0] seen_tag;
  integer selections;

  function automatic integer pick(input integer row);
    selections = selections + 1;
    return row;
  endfunction

  assign observed = cube[1][1][2];
  always_comb seen_tag = records[0][0].tag;

  initial begin
    cube[0][1] = '{7'd90, 7'd91, 7'd92};
    cube[1][1] = '{7'd11, 7'd12, 7'd13};
    records[0] = '{'{8'h11, 4'h1}, '{8'h22, 4'h2}, '{8'h33, 4'h3}};
    records[1] = '{'{8'h44, 4'h4}, '{8'h55, 4'h5}, '{8'h66, 4'h6}};
    #1;
    if (observed !== 7'd11 || seen_tag !== 8'h11)
      $fatal(1, "initial consumers");

    selections = 0;
    cube[pick(1)][1].reverse();
    records[pick(0)].reverse();
    #1;
    if (selections != 2 || observed !== 7'd13 || seen_tag !== 8'h33 ||
        cube[1][1][2] !== 7'd13 || cube[1][1][1] !== 7'd12 ||
        cube[1][1][0] !== 7'd11 || cube[0][1][2] !== 7'd90 ||
        records[0][0].payload !== 4'h3 || records[0][2].payload !== 4'h1 ||
        records[1][0].tag !== 8'h44 || records[1][2].payload !== 4'h6)
      $fatal(1, "reverse publication, row selection or record ownership");

    cube[pick(1)][1].reverse();
    records[pick(0)].reverse();
    #1;
    if (selections != 4 || observed !== 7'd11 || seen_tag !== 8'h11 ||
        cube[1][1][2] !== 7'd11 || records[0][0].payload !== 4'h1)
      $fatal(1, "reverse twice");
    $display("PASS syn027_reverse_notifications");
    $finish;
  end
endmodule

// IEEE 1800-2009 7.12.2: rows remain whole immediate elements when reordered.
module tb;
  typedef struct { byte key; bit flag; } record_t;
  record_t values[0:1][1:0];
  initial begin
    values[0][1].key = 8'd5; values[0][1].flag = 0;
    values[0][0].key = 8'd8; values[0][0].flag = 1;
    values[1][1].key = 8'd1; values[1][1].flag = 1;
    values[1][0].key = 8'd2; values[1][0].flag = 0;

    values.reverse();
    if (values[0][1].key !== 8'd1 || values[0][1].flag !== 1'b1 ||
        values[0][0].key !== 8'd2 || values[0][0].flag !== 1'b0 ||
        values[1][1].key !== 8'd5 || values[1][1].flag !== 1'b0 ||
        values[1][0].key !== 8'd8 || values[1][0].flag !== 1'b1)
      $fatal(1, "row reverse");

    values[0][1].key = 8'd9;
    values[1][1].key = 8'd0;
    values.sort() with (item[1].key);
    if (values[0][1].key !== 8'd0 || values[0][1].flag !== 1'b0 ||
        values[0][0].key !== 8'd8 || values[0][0].flag !== 1'b1 ||
        values[1][1].key !== 8'd9 || values[1][1].flag !== 1'b1 ||
        values[1][0].key !== 8'd2 || values[1][0].flag !== 1'b0)
      $fatal(1, "row sort");

    values.rsort() with (item[1].key);
    if (values[0][1].key !== 8'd9 || values[0][1].flag !== 1'b1 ||
        values[0][0].key !== 8'd2 || values[0][0].flag !== 1'b0 ||
        values[1][1].key !== 8'd0 || values[1][1].flag !== 1'b0 ||
        values[1][0].key !== 8'd8 || values[1][0].flag !== 1'b1)
      $fatal(1, "row rsort");

    $display("PASS r07_fixed_rows");
    $finish;
  end
endmodule

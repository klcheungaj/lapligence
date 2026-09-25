module tb;
  typedef struct { logic signed [7:0] key; bit [3:0] payload; } record_t;
  integer calls;

  function automatic integer pick(input integer row);
    calls = calls + 1;
    return row;
  endfunction

  task automatic exercise;
    record_t records[0:1][0:2];
    logic [7:0] cube[0:1][0:1][0:1];
    records[0] = '{'{8'sd3, 4'd3}, '{-8'sd1, 4'd1}, '{8'sd2, 4'd2}};
    records[1] = '{'{8'sd7, 4'd7}, '{8'sd8, 4'd8}, '{8'sd9, 4'd9}};
    calls = 0;
    records[pick(0)].sort() with (item.key);
    if (calls != 1 || records[0][0].key !== -8'sd1 || records[0][0].payload !== 4'd1 ||
        records[0][1].key !== 8'sd2 || records[0][1].payload !== 4'd2 ||
        records[0][2].key !== 8'sd3 || records[0][2].payload !== 4'd3)
      $fatal(1, "record map lost receiver or payload association");
    records[pick(0)].rsort() with (item.key);
    records[pick(0)].reverse();
    if (calls != 3 || records[0][0].key !== -8'sd1 || records[0][0].payload !== 4'd1 ||
        records[1][0].payload !== 4'd7 || records[1][2].payload !== 4'd9)
      $fatal(1, "record rsort/reverse or neighbor");
    cube[0] = '{'{8'd90, 8'd91}, '{8'd92, 8'd93}};
    cube[1] = '{'{8'd1, 8'd11}, '{8'd2, 8'd22}};
    cube[pick(1)].reverse();
    if (calls != 4 || cube[1][0][0] !== 8'd2 || cube[1][0][1] !== 8'd22 ||
        cube[1][1][0] !== 8'd1 || cube[1][1][1] !== 8'd11)
      $fatal(1, "reverse must permute whole rows");
    cube[pick(1)].sort() with (item[0]);
    cube[pick(1)].rsort() with (item[0]);
    if (calls != 6 || cube[1][0][1] !== 8'd22 || cube[1][1][1] !== 8'd11 ||
        cube[0][0][0] !== 8'd90 || cube[0][0][1] !== 8'd91 ||
        cube[0][1][0] !== 8'd92 || cube[0][1][1] !== 8'd93)
      $fatal(1, "row map receiver or neighbor");
  endtask

  initial begin
    exercise();
    $display("PASS n07_selected_record_and_row_ordering");
    $finish(0);
  end
endmodule

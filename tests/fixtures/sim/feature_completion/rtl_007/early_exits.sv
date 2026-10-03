// SV2009 13.4.1, 12.8, 9.6.2: return and local named-block disable leave
// fixed aggregate results, locals and output copy-out well defined.
module tb;
  typedef logic [7:0] arr_t [0:3];
  typedef struct { logic [7:0] row; logic [7:0] col; } pos_t;
  typedef logic [7:0] grid_t [0:2][0:2];

  function automatic arr_t first_ge(input arr_t x, input logic [7:0] lim);
    arr_t res;
    res = '{default: 8'hff};
    for (int i = 0; i < 4; i++) begin
      if (x[i] >= lim) begin
        res[0] = x[i];
        return res;
      end
      res[i] = 8'h00;
    end
    return res;
  endfunction

  function automatic int count_nonzero_prefix(input arr_t x);
    int n = 0;
    begin : scan
      foreach (x[i]) begin
        if (x[i] == 0) disable scan;
        n++;
      end
    end
    return n;
  endfunction

  function automatic pos_t find(input grid_t g, input logic [7:0] key);
    pos_t p;
    p = '{8'hff, 8'hff};
    for (int r = 0; r < 3; r++)
      for (int c = 0; c < 3; c++)
        if (g[r][c] == key) begin
          p = '{8'(r), 8'(c)};
          return p;
        end
    return p;
  endfunction

  task automatic count_until(input arr_t x, output int n, output arr_t seen);
    n = 0;
    seen = '{default: 8'h00};
    foreach (x[i]) begin
      if (x[i] == 8'hff) return;
      seen[i] = x[i];
      n++;
    end
  endtask

  task automatic skip_block(input arr_t x, output int n);
    n = 0;
    begin : body
      foreach (x[i]) begin
        if (x[i] == 8'h00) disable body;
        n += x[i];
      end
    end
    n = n + 100;
  endtask

  arr_t a, b;
  grid_t g;
  pos_t p;
  int n;
  initial begin
    a = '{8'd1, 8'd5, 8'd9, 8'd2};
    b = first_ge(a, 8'd4);
    $display("return_mid %h %h %h %h", b[0], b[1], b[2], b[3]);
    b = first_ge(a, 8'd100);
    $display("return_end %h %h %h %h", b[0], b[1], b[2], b[3]);
    a = '{8'd3, 8'd4, 8'd0, 8'd6};
    $display("disable_scan %0d", count_nonzero_prefix(a));
    foreach (g[r, c]) g[r][c] = 8'(r * 3 + c);
    p = find(g, 8'd7);
    $display("nested_return %0d %0d", p.row, p.col);
    p = find(g, 8'd20);
    $display("nested_miss %h %h", p.row, p.col);
    a = '{8'd3, 8'hff, 8'd0, 8'd6};
    count_until(a, n, b);
    $display("task_return %0d %h %h", n, b[0], b[1]);
    a = '{8'd3, 8'd4, 8'd0, 8'd6};
    skip_block(a, n);
    $display("task_disable %0d", n);
    $finish(0);
  end
endmodule

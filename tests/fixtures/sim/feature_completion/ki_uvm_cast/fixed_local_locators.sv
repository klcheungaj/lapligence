class P;
  int w = 3;
  function void top();
    int q[3];
    int qq[$];
    q = '{w, 9, 1};
    qq = q.max;
    $display("%0d %0d", qq.size(), qq[0]);
    qq = q.min;
    $display("%0d", qq[0]);
    qq = q.find_index with (item == 9);
    $display("%0d", qq[0]);
    qq = q.find with (item > 2);
    $display("%0d %0d", qq[0], qq[1]);
    qq = q.find_last_index with (item < 5);
    $display("%0d", qq[0]);
    begin
      int r[2];
      int m;
      int rr[$];
      r = '{w, 2};
      rr = r.max;
      m = rr[0];
      $display("%0d", m);
    end
  endfunction
  function void ranges();
    int d[3:1];
    int u[$];
    byte s[1:2];
    byte b[$];
    logic [3:0] x[2];
    logic [3:0] y[$];
    d = '{7, 8, 9};
    u = d.find_index with (item == 7);
    $display("%0d", u[0]);
    s = '{-1, 2};
    b = s.min;
    $display("%0d", b[0]);
    x = '{4'hc, 4'h3};
    y = x.unique;
    $display("%0d %h %h", y.size(), y[0], y[1]);
  endfunction
endclass
module tb;
  task automatic t();
    int q[2];
    int r[$];
    q = '{5, 6};
    #1;
    r = q.max;
    $display("%0d", r[0]);
  endtask
  P p = new;
  initial begin
    p.top();
    p.ranges();
    t();
    $finish;
  end
endmodule

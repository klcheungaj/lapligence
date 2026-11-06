// A `rand` fixed array with several unpacked dimensions (18.4, 18.5.8.1):
// every element is random, `foreach` walks one or several dimensions from
// the left, an omitted loop variable skips its dimension, a row has
// reductions, a random index selects any legal element and an index
// outside its own dimension is an error, not another row's element.
class matrix_c;
  rand bit [3:0] m[2][3];
  rand bit [3:0] down[1:2][3:1];
  rand bit [1:0] x, y;
  rand byte s[3][2];
  int row = 1;
  rand bit [3:0] w[2][2][2];
  constraint rows { foreach (m[i]) m[i].sum() with (int'(item)) == 10; foreach (m[i, j]) m[i][j] < 7; }
  constraint nested { foreach (down[i]) foreach (down[i][j]) down[i][j] == i * 10 % 16 + j; }
  constraint picked { m[x][y] == 6; }
  constraint state_index { s[row][0] == 5; foreach (s[, j]) s[0][j] == j; }
  constraint skipped { foreach (w[i, , k]) w[i][0][k] == i + k; w[1][1][1] inside {[1:3]}; }
endclass

class member_c;
  rand matrix_c a;
  rand bit [3:0] copy;
  constraint c { a.m[1][2] == copy; copy == 4; }
  function new(); a = new; endfunction
endclass

module tb;
  initial begin
    static matrix_c o = new;
    static member_c h = new;
    int picks[4][4];
    int total, bad;
    repeat (300) begin
      if (o.randomize() != 1) $display("failed");
      foreach (o.m[i]) begin
        total = 0;
        foreach (o.m[i][j]) begin
          total += o.m[i][j];
          if (o.m[i][j] >= 7) bad++;
        end
        if (total != 10) bad++;
      end
      if (o.x > 1 || o.y > 2 || o.m[o.x][o.y] != 6) bad++;
      picks[o.x][o.y]++;
      if (o.s[1][0] != 5 || o.s[0][0] != 0 || o.s[0][1] != 1) bad++;
      if (!(o.w[1][1][1] inside {1, 2, 3})) bad++;
    end
    $display("bad=%0d", bad);
    $display("%p", o.down);
    $display("%0d %0d %0d", o.w[1][0][1], o.w[0][0][1], o.w[1][0][0]);
    foreach (picks[i, j]) if (picks[i][j] != 0) $display("pick %0d %0d", i, j);
    $display("%0d %0d %0d", h.randomize(), h.a.m[1][2], h.copy);
    o.row = 5;
    o.constraint_mode(0);
    o.state_index.constraint_mode(1);
    $display("%0d", o.randomize());
    $finish(0);
  end
endmodule

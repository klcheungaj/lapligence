`timescale 1ns/1ns
class C;
  int v;
  function new(int v); this.v = v; endfunction
endclass
module tb;
  int a[3];
  int q[$];
  int dy[];
  int aq[string][$];
  int ac[C];
  int empty[int];
  logic clk = 0;
  task automatic ta(int n);
    int loc[2];
    foreach (loc[i]) fork
      $display("(B) task n=%0d i=%0d", n, i);
    join_none
  endtask
  task ts();
    int loc[string];
    loc["p"] = 1; loc["q"] = 2;
    foreach (loc[s]) fork
      #1 $display("(B) static task s=%s", s);
    join_none
  endtask
  function automatic void fa();
    int loc[2];
    foreach (loc[i]) fork
      $display("(B) function i=%0d", i);
    join_none
  endfunction
  initial begin
    C c1, c2;
    q = '{10, 20, 30};
    dy = new[2];
    aq["x"] = '{1, 2}; aq["y"] = '{3};
    c1 = new(1); c2 = new(2);
    ac[c1] = 10; ac[c2] = 20;
    // (A) nested loops, a nested keyed level and a class key
    foreach (a[i]) begin
      foreach (q[j]) fork
        $display("(A) nested i=%0d j=%0d", i, j);
      join_none
    end
    #1;
    foreach (aq[s, j]) fork
      $display("(A) string key s=%s j=%0d", s, j);
    join_none
    #1;
    foreach (ac[k]) fork
      $display("(A) class key k.v=%0d", k.v);
    join_none
    #1;
    foreach (dy[i]) fork
      $display("(A) dynamic i=%0d", i);
    join_none
    #1;
    foreach (empty[k]) fork
      $display("(A) empty k=%0d", k);
    join_none
    #1;
    // (B) subroutine loops
    ta(7);
    #1;
    ts();
    #2;
    fa();
    #1;
    // (C) a fork-local copy, waits on the shared variable, a named fork
    foreach (a[i]) fork
      automatic int k = i;
      #1 $display("(C) copy k=%0d i=%0d", k, i);
    join_none
    #2;
    foreach (a[i]) begin
      fork
        begin
          wait (i == 3);
          $display("(C) waited i=%0d", i);
        end
      join_none
      #1;
    end
    #1;
    foreach (aq[s]) fork
      begin
        wait (s == "y");
        $display("(C) waited s=%s", s);
      end
    join_none
    #1;
    foreach (a[i]) fork : named
      $display("(C) named i=%0d", i);
    join_none
    #1;
    // (D) a fork joined before the next iteration reads each index
    foreach (a[i]) begin
      fork
        $display("(D) joined i=%0d", i);
      join
      fork
        $display("(D) join_none i=%0d", i);
      join_none
      #0 $display("(D) after #0 i=%0d", i);
    end
    #1;
    clk = 1;
    #1 clk = 0;
    #1 clk = 1;
    #2 $finish;
  end
  // (E) each activation of a block that runs again has its own loop
  always @(posedge clk) begin
    foreach (a[i]) fork
      #1 $display("(E) always i=%0d t=%0t", i, $time);
    join_none
  end
endmodule

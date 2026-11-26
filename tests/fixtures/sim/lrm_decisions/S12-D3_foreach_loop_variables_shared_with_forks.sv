// Decision S12-D3: a `join_none` or `join_any` branch that names a `foreach`
// loop variable shares the loop's one variable, as it shares a variable
// declared in a for-loop's initialization. 12.7.3
// (SystemVerilog-1800-2009.txt L18211-18213): "As in a for-loop (12.7.1), a
// foreach-loop creates an implicit begin-end block around the loop statement,
// containing declarations of the loop variables with automatic lifetime. This
// block creates a new hierarchical scope, making the variables local to the
// loop scope." The value a branch reads after the loop has ended is not in the
// text: like a for-loop whose test fails (12.7.1 L18069-18075), an index
// variable is one step past the last index it visited (one past the right
// bound of a fixed dimension, in its direction, or the size of a queue,
// dynamic array or string), and an associative key keeps the last key, as
// next() does when no entry follows (7.9.6 L8880-8881: "Otherwise, the index
// is unchanged, and the function returns 0.").
module tb;
  int up[3];
  int down[2:0];
  int q[$];
  int aa[int];
  int as[string];
  int m[2][3];
  string word = "ab";
  initial begin
    q = '{10, 20};
    aa[5] = 1; aa[9] = 2;
    as["x"] = 1; as["y"] = 2;
    foreach (up[i]) fork
      $display("up %0d", i);
    join_none
    #1;
    foreach (down[i]) fork
      $display("down %0d", i);
    join_none
    #1;
    foreach (q[i]) fork
      $display("queue %0d", i);
    join_none
    #1;
    foreach (word[i]) fork
      $display("string %0d", i);
    join_none
    #1;
    foreach (aa[k]) fork
      $display("int key %0d", k);
    join_none
    #1;
    foreach (as[s]) fork
      $display("string key %s", s);
    join_none
    #1;
    foreach (m[i, j]) fork
      $display("2-D %0d %0d", i, j);
    join_none
    #1;
    foreach (up[i]) fork
      #1 $display("join_any late %0d", i);
      $display("join_any first %0d", i);
    join_any
    #2 $finish;
  end
endmodule

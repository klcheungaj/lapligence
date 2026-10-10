// SIM-014: intra-assignment event and repeated event controls inside task
// bodies (SV 9.4.5, 13.3). Automatic and recursive tasks wait in their own
// activation and write their own locals; a static task's nonblocking forms
// target module storage and capture their RHS and selectors at issue.
`timescale 1ns / 1ns
module tb;
  event e;
  int g, h;
  logic [7:0] arr[0:3];
  string label;

  // `e` occurs at 2, 4, 6, ...
  initial forever #2 ->e;

  task automatic hold(int n, int v);
    int loc;
    string text;
    loc = repeat (n) @e v;
    text = @e $sformatf("v%0d", loc);
    g = loc;
    $display("%0t hold text=%s", $time, text);
  endtask

  task automatic digits(int d);
    int loc;
    if (d > 0) begin
      loc = @e d;
      h = h * 10 + loc;
      digits(d - 1);
    end
  endtask

  task queue_writes(int n, int i, logic [7:0] v, string s);
    arr[i] <= repeat (n) @e v;
    label <= @e s;
  endtask

  initial begin
    foreach (arr[k]) arr[k] = 0;
    #1 hold(2, 5);
    $display("%0t g=%0d", $time, g);
    #1 digits(3);
    $display("%0t h=%0d", $time, h);
    #1 queue_writes(2, 1, 8'h3c, "late");
    $display("%0t issued arr[1]=%h label=%s", $time, arr[1], label);
    #5 $display("%0t arr[1]=%h label=%s", $time, arr[1], label);
    $finish;
  end
endmodule

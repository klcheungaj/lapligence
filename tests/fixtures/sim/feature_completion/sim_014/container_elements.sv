// SIM-014 A01: blocking timed writes of resizable-container and string-array
// elements whose index or key changes while the write is pending. The value
// is captured before the timing control and the element is selected when it
// completes (SV 9.4.5, 10.4.1, 4.9.3).
`timescale 1ns / 1ns
module tb;
  class Box;
    int id;
    function new(int i);
      id = i;
    endfunction
  endclass
  event e;
  int i;
  string key, text;
  string names[0:1];
  string sq[$];
  real ar[string];
  Box boxes[];
  Box b1;

  // `e` occurs at 4, 8, 12, ...
  initial forever #4 ->e;

  initial begin
    names[0] = "";
    names[1] = "";
    sq = {"", "", ""};
    boxes = new[2];
    #1 i = 0;
    text = "hi";
    fork
      names[i] = @e text;
      #1 begin
        i = 1;
        text = "no";
      end
    join
    #1 $display("%0t names: '%s' '%s'", $time, names[0], names[1]);
    i = 0;
    fork
      sq[i] = repeat (2) @e "q";
      #1 i = 2;
    join
    #1 $display("%0t sq: '%s' '%s' '%s'", $time, sq[0], sq[1], sq[2]);
    key = "a";
    fork
      ar[key] = #2 1.25;
      #1 key = "b";
    join
    #1 $display("%0t ar: %0d %0.2f", $time, ar.exists("a"), ar["b"]);
    #1 i = 0;
    b1 = new(7);
    fork
      boxes[i] = @e b1;
      #1 begin
        i = 1;
        b1 = null;
      end
    join
    #1 $display("%0t boxes: %0d %0d", $time, boxes[0] == null, boxes[1].id);
    $finish;
  end
endmodule

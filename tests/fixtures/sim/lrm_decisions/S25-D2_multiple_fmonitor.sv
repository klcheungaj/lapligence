// Decision S25-D2: any number of $fmonitor lists are active at once,
// independent of $monitor; $fclose cancels a multichannel list only for the
// channel it closes.
//
// IEEE 1800-2009 21.3.2 (SystemVerilog-1800-2009.txt L36676-36679):
//   "Unlike $monitor, any number of $fmonitor tasks can be set up to be
//   simultaneously active. However, there is no counterpart to $monitoron
//   and $monitoroff tasks. The task $fclose is used to cancel an active
//   $fstrobe or $fmonitor task."
// IEEE 1800-2009 21.3.1 (L36635-36636):
//   "Active $fmonitor and/or $fstrobe operations on a file descriptor or
//   multichannel descriptor are implicitly cancelled by an $fclose
//   operation."
//
// The text does not say what closing one channel of a multichannel list does
// to its other channels; llg cancels the list only for the closed channel
// (the "both" list below keeps writing to file 2 after file 1 is closed).
// Every list watches its own signal and changes in its own slot, so the
// file contents do not depend on the order of reports within a slot.
module tb;
  integer f1, f2;
  reg [3:0] a = 1, b = 4, c = 1;

  task static dump(input string name);
    integer fd, k;
    string text;
    begin
      fd = $fopen(name, "r");
      k = $fgets(text, fd);
      while (k != 0) begin
        $write("%s: %s", name, text);
        k = $fgets(text, fd);
      end
      $fclose(fd);
    end
  endtask

  initial begin
    f1 = $fopen("s25d2_1.txt");
    f2 = $fopen("s25d2_2.txt");
    $monitor("mon a=%0d b=%0d c=%0d", a, b, c);
    $fmonitor(f1, "one a=%0d", a);
    #1 b = 5;
    $fmonitor(f2, "two b=%0d", b);
    #1 $fmonitor(f1 | f2, "both c=%0d", c);
    #1 a = 2;
    #1 b = 6;
    #1 c = 2;
    #1 $fclose(f1);
    #1 c = 3;
    #1 a = 3; b = 7;
    #1 $fclose(f2);
    #1 c = 4;
    #1 dump("s25d2_1.txt");
    dump("s25d2_2.txt");
    $finish;
  end
endmodule

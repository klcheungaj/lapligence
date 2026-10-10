// SIM-025 A02: $fstrobe on file, multichannel and closed descriptors
// (SV 21.2.2, 21.3.1, 21.3.2). $fclose cancels a pending $fstrobe of the
// channel it closes; a multichannel strobe keeps its remaining channels; a
// channel reopened in the same slot only receives its own strobes.
module tb;
  integer f1, f2, f3, mc;
  reg [3:0] a = 1;

  task static dump(input string name);
    integer fd, n;
    string line;
    begin
      fd = $fopen(name, "r");
      n = $fgets(line, fd);
      while (n != 0) begin
        $write("%s: %s", name, line);
        n = $fgets(line, fd);
      end
      $fclose(fd);
    end
  endtask

  initial begin
    f1 = $fopen("sim025_s1.txt");
    f2 = $fopen("sim025_s2.txt");
    mc = f1 | f2 | 1;
    $fstrobe(f1, "s1 a=%0d", a);
    $fstrobe(mc, "mc a=%0d", a);
    $fstrobe(f2, "gone a=%0d", a);
    a = 2;
    $fclose(f2);
    f3 = $fopen("sim025_s3.txt");
    $fstrobe(f3, "new a=%0d", a);
    #1;
    $fclose(f1);
    $fclose(f3);
    dump("sim025_s1.txt");
    dump("sim025_s2.txt");
    dump("sim025_s3.txt");
    $finish(0);
  end
endmodule

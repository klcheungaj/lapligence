// SIM-025 A02: any number of $fmonitor tasks are active at once, each on its
// own descriptor, independent of $monitor; $fclose cancels the monitors of
// the channels it closes (SV 21.3.1, 21.3.2). A reopened channel never
// receives output of a monitor of its previous file.
module tb;
  integer f1, f2, f3, f4, f5, mc;
  reg [3:0] a = 1, b = 2;

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
    f1 = $fopen("sim025_m1.txt");
    f2 = $fopen("sim025_m2.txt");
    f3 = $fopen("sim025_m3.txt");
    f5 = $fopen("sim025_m5.txt", "w");
    mc = f2 | f3;
    $fmonitor(f1, "f1 a=%0d", a);
    $fmonitor(f1, "f1b b=%0d", b);
    $fmonitor(f2, "f2 a=%0d b=%0d", a, b);
    $fmonitor(f3, "f3 b=%0d", b);
    $fmonitor(mc, "mc a=%0d", a);
    $fmonitor(1, "out a=%0d", a);
    $monitor("mon b=%0d", b);
    $fmonitor(f5, "fd b=%0d", b);
    #1 a = 2;
    #1 b = 3;
    #1 $fclose(f1);
    f4 = $fopen("sim025_m4.txt");
    #1 a = 4; b = 4;
    #1 $fclose(f2);
    $fclose(f3);
    $fmonitor(f4, "f4 a=%0d", a);
    #1 a = 5; b = 5;
    #1 $fclose(f4);
    $fclose(f5);
    dump("sim025_m1.txt");
    dump("sim025_m2.txt");
    dump("sim025_m3.txt");
    dump("sim025_m4.txt");
    dump("sim025_m5.txt");
    $finish(0);
  end
endmodule

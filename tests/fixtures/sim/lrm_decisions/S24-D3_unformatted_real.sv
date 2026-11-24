// Decision S24-D3: a real argument that no format specification consumes
// prints in the shortest form of C's %g, as %p prints it (S24-D2): six
// significant digits, an exponent outside 1e-4..1e6 and no trailing zeros
// (1.5, 2, 0.1, 1e+10, 1.23457e+06, -0, inf). It takes no field width and no
// radix: the b, o and h variants print the same text. Every display, strobe,
// monitor, file and string output task agrees, whatever stands beside the
// real in the argument list. %e, %f and %g keep their own forms.
//
// IEEE 1800-2009 21.2.1.2 (SystemVerilog-1800-2009.txt L35976-35978):
//   "Any expression argument that has no corresponding format specification
//   is displayed using the default decimal format in $display and $write,
//   binary format in $displayb and $writeb, octal format in $displayo and
//   $writeo, and hexadecimal format in $displayh and $writeh."
//
// A decimal format of a real is not defined; llg follows industry-standard
// behaviour.
module tb;
  typedef struct { real r; int i; } rec_t;
  class box;
    real r = 1.25;
    function real half(); return r / 2.0; endfunction
  endclass
  real r = 1.5, zero = 0.0, big, ra[2];
  shortreal s = 0.1, third = 1.0 / 3.0;
  realtime rt = 12.5;
  real q[$], tab[$];
  rec_t rec;
  box b;
  string text;
  reg [8*8-1:0] packed_text;
  integer fd, code;
  parameter real PR = 3.0;
  localparam LPR = 0.1;
  function real fr(int x); return x / 2.0; endfunction
  initial begin
    b = new;
    rec.r = 9.0;
    ra[0] = 7.0;
    ra[1] = 7.5;
    q.push_back(8.25);
    // Alone, beside literals and other arguments, and after conversions.
    $display(r);
    $display("", r);
    $display("x=", r, " y=", 2.0, ".");
    $display(r, r);
    $display(r,, r);
    $display(5, r, 4'd3, 8'sd3, "|");
    $display("%d", 5, r);
    $display("%g %f %e", r, r, r, r);
    $display(r, "%g", r);
    // The variants have no radix for a real.
    $displayb(r, 2'b1x);
    $displayo(r);
    $displayh("h", r);
    $write(r, "\n");
    $writeh("w", r, "\n");
    // Kinds of real operands.
    $display(2.0, " ", PR, " ", LPR, " ", 1.0 / 3.0, " ", -r, " ", 1.5 + 1);
    $display(s, " ", third, " ", rt, " ", ra[0], " ", ra[1], " ", q[0]);
    $display(rec.r, " ", b.r, " ", b.half(), " ", fr(3), " ", fr(4));
    $display(real'(3), " ", $itor(3), " ", $sqrt(2.0), " ", shortreal'(0.1));
    // Magnitudes.
    tab = '{0.0, -0.0, 1.0, -1.0, 100.0, 100000.0, 123456.0, 999999.5, 1e6,
            1234567.0, 1e10, 1e15, 1e16, 1e20, 1e100, 1e300, 0.5, 0.01, 0.001,
            0.0001, 0.00001, 1e-10, 1e-300, 3.14159265358979, 123456.789,
            99999.95, 1.23456789e-7, 1.7976931348623157e308, -123.456};
    foreach (tab[k]) $display(tab[k]);
    big = 1.0 / zero;
    $display(big, " ", -big);
    // String targets.
    $swrite(text, r);
    $display("<%s>", text);
    $swrite(text, "q", r, "q", 2.0);
    $display("<%s>", text);
    $swriteh(text, r);
    $display("<%s>", text);
    $sformat(text, "%0d:", 3, r);
    $display("<%s>", text);
    text = $sformatf("%0d:", 3, 2.0);
    $display("<%s>", text);
    $swrite(packed_text, "r=", r);
    $display("%0s", packed_text);
    // Files.
    fd = $fopen("S24-D3_unformatted_real.txt", "w");
    $fwrite(fd, r, "x");
    $fdisplay(fd, 2.0, " ", 1e10);
    $fclose(fd);
    fd = $fopen("S24-D3_unformatted_real.txt", "r");
    code = $fgets(text, fd);
    $fclose(fd);
    $write("file %s", text);
    $fdisplay(1, "f", r);
    // Postponed output.
    $strobe("strobe ", r);
    #1;
    $monitor("monitor ", r, " ", s);
    #1 r = 2.0;
    #1 s = 1e10;
    #1 $monitoroff;
    $finish;
  end
endmodule

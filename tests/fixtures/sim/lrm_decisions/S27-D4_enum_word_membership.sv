// Decision S27-D4: a word loaded into an enum element must be the value of
// one of the enum's members. The comparison uses the base type's width: a
// number with more digits is a member only when its extra bits are zero
// (unsigned base) or repeat the base's sign bit (signed base), and a word
// with x or z bits is a member only when a member has exactly that value.
// Any other word is an error that ends the load; the words before it stay.
//
// IEEE 1800-2009 21.4.2 (SystemVerilog-1800-2009.txt L37368-37370):
//   "For enumerated types, the file data represents the numeric values
//   associated with each element of the enumerated type (see 6.19). If a
//   numeric value is out of range for a given type, then an error shall be
//   issued and no further reading shall take place."
//
// The text does not say how a number wider than the base type, or one with
// x or z bits, is compared with the member values; this is llg's reading.
module tb;
  typedef enum logic signed [3:0] {NEG = -1, ZERO = 0, ONE = 1} signed_t;
  typedef enum logic [3:0] {LOW = 4'h1, UNKNOWN = 4'hx} four_state_t;
  signed_t s [0:3];
  four_state_t f [0:3];
  integer fd;
  initial begin
    fd = $fopen("s27_d4_signed.hex", "w");
    $fwrite(fd, "f\nff\n01\n1f\n");
    $fclose(fd);
    fd = $fopen("s27_d4_four.hex", "w");
    $fwrite(fd, "1\nx\nz\n1\n");
    $fclose(fd);
    s = '{ZERO, ZERO, ZERO, ZERO};
    f = '{LOW, LOW, LOW, LOW};
    $readmemh("s27_d4_signed.hex", s);
    $readmemh("s27_d4_four.hex", f);
    $display("signed %s %s %s %s", s[0].name(), s[1].name(), s[2].name(), s[3].name());
    $display("four_state %s %s %s %s", f[0].name(), f[1].name(), f[2].name(), f[3].name());
    $finish;
  end
endmodule

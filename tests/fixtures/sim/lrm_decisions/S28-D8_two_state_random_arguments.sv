// IEEE 1800-2009 18.13.3 L31086: "function void srandom( int seed );";
// 18.13.1 L31018: "function int unsigned $urandom [ (int seed ) ] ;";
// 18.13.2 L31044-31045: "function int unsigned $urandom_range( int unsigned
// maxval, int unsigned minval = 0 );". 6.11.2 L5489-5490: "When a 4-state
// value is automatically converted to a 2-state value, any unknown or
// high-impedance bits shall be converted to zeros."
// Decision (LRM text): X/Z bits in these 2-state arguments read as 0.
class item_c;
  int k;
endclass

module tb;
  logic [31:0] unknown;
  int unsigned x, y;
  item_c a, b;
  initial begin
    unknown = 'x;
    process::self().srandom(unknown);
    x = $urandom;
    process::self().srandom(0);
    y = $urandom;
    $display("srandom %0d", x == y);
    x = $urandom(unknown);
    y = $urandom(0);
    $display("urandom seed %0d", x == y);
    process::self().srandom(5);
    x = $urandom_range(unknown, 3);
    process::self().srandom(5);
    y = $urandom_range(0, 3);
    $display("urandom_range %0d", x == y);
    a = new;
    b = new;
    a.srandom(unknown);
    b.srandom(0);
    $display("object srandom %0d", a.get_randstate() == b.get_randstate());
    $finish;
  end
endmodule

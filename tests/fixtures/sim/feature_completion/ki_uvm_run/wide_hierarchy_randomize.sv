// A class with 40 directly derived classes and a two-level chain under
// one of them, randomized through base-class handles and a `rand` handle:
// the constraints of the object's own class hold, most derived first.
class item;
  rand int unsigned x;
  constraint c_x { x < 100; }
endclass

class d0 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 0; }
endclass

class d1 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 1; }
endclass

class d2 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 2; }
endclass

class d3 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 3; }
endclass

class d4 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 4; }
endclass

class d5 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 5; }
endclass

class d6 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 6; }
endclass

class d7 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 7; }
endclass

class d8 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 8; }
endclass

class d9 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 9; }
endclass

class d10 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 10; }
endclass

class d11 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 11; }
endclass

class d12 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 12; }
endclass

class d13 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 13; }
endclass

class d14 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 14; }
endclass

class d15 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 15; }
endclass

class d16 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 16; }
endclass

class d17 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 17; }
endclass

class d18 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 18; }
endclass

class d19 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 19; }
endclass

class d20 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 20; }
endclass

class d21 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 21; }
endclass

class d22 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 22; }
endclass

class d23 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 23; }
endclass

class d24 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 24; }
endclass

class d25 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 25; }
endclass

class d26 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 26; }
endclass

class d27 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 27; }
endclass

class d28 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 28; }
endclass

class d29 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 29; }
endclass

class d30 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 30; }
endclass

class d31 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 31; }
endclass

class d32 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 32; }
endclass

class d33 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 33; }
endclass

class d34 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 34; }
endclass

class d35 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 35; }
endclass

class d36 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 36; }
endclass

class d37 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 37; }
endclass

class d38 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 38; }
endclass

class d39 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 39; }
endclass

class e1 extends d5;
  rand int unsigned z;
  constraint c_z { z == y + 1; }
endclass

class e2 extends e1;
  rand int unsigned w;
  constraint c_w { w == z + 1; }
endclass

class holder;
  rand item it;
  function new(item i);
    it = i;
  endfunction
endclass

module tb;
  item h;
  holder o;
  d0 a0;
  d39 a1;
  e2 b;
  item base;
  int ok;

  initial begin
    a0 = new;
    a1 = new;
    b = new;
    base = new;
    h = a1;
    ok = h.randomize();
    $display("d39 %0d %0d", ok, a1.x < 100 && a1.y == a1.x + 39);
    h = b;
    if (!h.randomize())
      $display("e2 failed");
    $display("e2 %0d", b.x < 100 && b.y == b.x + 5 && b.z == b.y + 1 && b.w == b.z + 1);
    h = base;
    ok = h.randomize();
    $display("item %0d %0d", ok, base.x < 100);
    o = new(a0);
    ok = o.randomize();
    $display("holder d0 %0d %0d", ok, a0.x < 100 && a0.y == a0.x);
    o.it = b;
    ok = o.randomize();
    $display("holder e2 %0d %0d %0d", ok, o.it == b, b.x < 100 && b.y == b.x + 5 && b.w == b.z + 1);
    $finish;
  end
endmodule

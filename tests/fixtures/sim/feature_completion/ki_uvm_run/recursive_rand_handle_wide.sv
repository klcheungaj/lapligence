// Generation only: a base class with a recursive `rand` handle and 20 derived classes, each
// adding a `rand` variable and a constraint (18.4: objects reached through
// `rand` handles are randomized with their parent, recursively).
class item;
  rand int unsigned x;
  rand item next;
  constraint c_x { x < 1000; }
  virtual function bit ok(); return x < 1000; endfunction
  virtual function int unsigned kind(); return 1000; endfunction
endclass
class d0 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 0; }
  virtual function bit ok(); return x < 1000 && y == x + 0; endfunction
  virtual function int unsigned kind(); return 0; endfunction
endclass
class d1 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 1; }
  virtual function bit ok(); return x < 1000 && y == x + 1; endfunction
  virtual function int unsigned kind(); return 1; endfunction
endclass
class d2 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 2; }
  virtual function bit ok(); return x < 1000 && y == x + 2; endfunction
  virtual function int unsigned kind(); return 2; endfunction
endclass
class d3 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 3; }
  virtual function bit ok(); return x < 1000 && y == x + 3; endfunction
  virtual function int unsigned kind(); return 3; endfunction
endclass
class d4 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 4; }
  virtual function bit ok(); return x < 1000 && y == x + 4; endfunction
  virtual function int unsigned kind(); return 4; endfunction
endclass
class d5 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 5; }
  virtual function bit ok(); return x < 1000 && y == x + 5; endfunction
  virtual function int unsigned kind(); return 5; endfunction
endclass
class d6 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 6; }
  virtual function bit ok(); return x < 1000 && y == x + 6; endfunction
  virtual function int unsigned kind(); return 6; endfunction
endclass
class d7 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 7; }
  virtual function bit ok(); return x < 1000 && y == x + 7; endfunction
  virtual function int unsigned kind(); return 7; endfunction
endclass
class d8 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 8; }
  virtual function bit ok(); return x < 1000 && y == x + 8; endfunction
  virtual function int unsigned kind(); return 8; endfunction
endclass
class d9 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 9; }
  virtual function bit ok(); return x < 1000 && y == x + 9; endfunction
  virtual function int unsigned kind(); return 9; endfunction
endclass
class d10 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 10; }
  virtual function bit ok(); return x < 1000 && y == x + 10; endfunction
  virtual function int unsigned kind(); return 10; endfunction
endclass
class d11 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 11; }
  virtual function bit ok(); return x < 1000 && y == x + 11; endfunction
  virtual function int unsigned kind(); return 11; endfunction
endclass
class d12 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 12; }
  virtual function bit ok(); return x < 1000 && y == x + 12; endfunction
  virtual function int unsigned kind(); return 12; endfunction
endclass
class d13 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 13; }
  virtual function bit ok(); return x < 1000 && y == x + 13; endfunction
  virtual function int unsigned kind(); return 13; endfunction
endclass
class d14 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 14; }
  virtual function bit ok(); return x < 1000 && y == x + 14; endfunction
  virtual function int unsigned kind(); return 14; endfunction
endclass
class d15 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 15; }
  virtual function bit ok(); return x < 1000 && y == x + 15; endfunction
  virtual function int unsigned kind(); return 15; endfunction
endclass
class d16 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 16; }
  virtual function bit ok(); return x < 1000 && y == x + 16; endfunction
  virtual function int unsigned kind(); return 16; endfunction
endclass
class d17 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 17; }
  virtual function bit ok(); return x < 1000 && y == x + 17; endfunction
  virtual function int unsigned kind(); return 17; endfunction
endclass
class d18 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 18; }
  virtual function bit ok(); return x < 1000 && y == x + 18; endfunction
  virtual function int unsigned kind(); return 18; endfunction
endclass
class d19 extends item;
  rand int unsigned y;
  constraint c_y { y == x + 19; }
  virtual function bit ok(); return x < 1000 && y == x + 19; endfunction
  virtual function int unsigned kind(); return 19; endfunction
endclass
class e0 extends d0;
  rand int unsigned z;
  constraint c_z { z == y + 1; }
  virtual function bit ok(); return super.ok() && z == y + 1; endfunction
  virtual function int unsigned kind(); return 2000; endfunction
endclass
module tb;
  item h, p;
  int count;
  item o0, o1, o2, o3, o4, o5;
  initial begin
    begin d3 t; t = new; o0 = t; end
    begin d19 t; t = new; o1 = t; end
    o2 = new;
    begin e0 t; t = new; o3 = t; end
    o4 = new;
    begin d10 t; t = new; o5 = t; end
    o0.next = o1;
    o1.next = o2;
    o2.next = o3;
    o3.next = o4;
    o4.next = o5;
    h = o0;
    repeat (3) begin
      if (!h.randomize()) $display("randomize failed");
      count = 0;
      for (p = h; p != null; p = p.next) begin
        $display("kind %0d ok %0d", p.kind(), p.ok());
        count++;
      end
      $display("linked %0d", count);
    end
    $finish;
  end
endmodule

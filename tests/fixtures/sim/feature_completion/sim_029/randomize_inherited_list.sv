// randomize(list) names properties of the object (18.11), inherited ones
// included: only those change, through this call, a handle or a base handle.
class b_c;
  rand int x;
  rand int y;
  constraint c { x > 0; x < 5; }
endclass

class d_c extends b_c;
  rand int z;
  constraint d { z == 7 || z == 0; }
  function void go();
    y = 99;
    $display("%0d %0d %0d %0d", randomize(x), x > 0 && x < 5, y, z);
  endfunction
endclass

module tb;
  initial begin
    static d_c d = new;
    b_c b;
    d.go();
    $display("%0d %0d", d.randomize(y), d.z);
    b = d;
    $display("%0d %0d", b.randomize(x), b.x > 0 && b.x < 5);
    $finish(0);
  end
endmodule

// IEEE 1800-2009 11.4.13: set membership over fixed arrays. RHS X/Z bits are
// wildcards; a known match dominates an X comparison; arrays (stored, selected
// rows, call results) contribute their elements, captured once.
module tb;
  typedef int row_t [2];
  logic [3:0] a [3];
  int b [2][2];
  logic [3:0] wide [0:39];
  logic [7:0] rows [0:2][0:23];
  integer sel;
  int calls;

  function automatic row_t g(int x);
    row_t r = '{x, x + 1};
    calls++;
    return r;
  endfunction

  initial begin
    a = '{4'b1x00, 4'd5, 4'd9};
    $display("1:%b", 4'd5 inside {a});
    $display("2:%b", 4'd7 inside {a});
    $display("3:%b", 4'b1100 inside {a});
    $display("4:%b", 4'b1x00 inside {4'd5});
    $display("5:%b", 4'b0x01 inside {4'd5, 4'd6});
    $display("6:%b", 4'b0x01 inside {4'd5, 4'b0001});
    $display("7:%b", 4'b0x01 inside {4'd5, 4'b0z01});
    b = '{'{1, 2}, '{3, 4}};
    $display("8:%b %b", 3 inside {b}, 5 inside {b});
    calls = 0;
    $display("9:%b %b", 4 inside {g(3)}, 6 inside {g(3)});
    $display("calls=%0d", calls);
    $display("10:%b", 2 inside {b[0]});
    $display("11:%b", -1 inside {b, [-3:-1]});
    foreach (wide[i]) wide[i] = 4'(i);
    wide[17] = 4'bxx11;
    $display("12:%b %b", 4'd3 inside {wide}, 4'bx011 inside {wide[20:39]});
    foreach (rows[i, j]) rows[i][j] = 8'(i * 30 + j);
    sel = 1;
    $display("13:%b %b", 8'd53 inside {rows[sel]}, 8'd60 inside {rows[sel]});
    rows[2][5] = 8'bx;
    sel = 2;
    $display("14:%b %b", 8'd77 inside {rows[sel]}, 8'd200 inside {rows[sel]});
    sel = 'x;
    $display("15:%b", 8'd0 inside {rows[sel]});
    $finish(0);
  end
endmodule

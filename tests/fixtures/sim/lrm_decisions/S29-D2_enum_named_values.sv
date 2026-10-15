// IEEE 1800-2009 18.3 L29224-29225: constraints use "variables and constants
// of integral type (e.g., bit, reg, logic, integer, enum, packed struct)".
// The text does not say whether a random enum variable may take a value of
// its base type that names no member.
// Decision (llg choice, common practice): a rand enum variable takes only its
// named values, as if constrained by `inside` over its members.
typedef enum bit [2:0] { RED = 3'd1, GREEN = 3'd4, BLUE = 3'd6 } color_e;
class pick_c;
  rand color_e color;
  rand bit [2:0] other;
  constraint c { other == 3'(color); }
endclass
module tb;
  initial begin
    static pick_c pick = new;
    int unnamed, seen_red, seen_green, seen_blue;
    unnamed = 0;
    seen_red = 0;
    seen_green = 0;
    seen_blue = 0;
    repeat (1000) begin
      void'(pick.randomize());
      case (pick.color)
        RED: seen_red = 1;
        GREEN: seen_green = 1;
        BLUE: seen_blue = 1;
        default: unnamed++;
      endcase
      if (pick.other != 3'(pick.color)) unnamed++;
    end
    $display("unnamed=%0d red=%0d green=%0d blue=%0d", unnamed, seen_red, seen_green, seen_blue);
    $finish;
  end
endmodule

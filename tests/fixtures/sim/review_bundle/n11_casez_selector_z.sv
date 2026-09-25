// Shared V2001/SV2009 grammar: 9.5.1 / 12.5.1 make Z a wildcard on either side.
module tb;
  reg constant_hit;
  wire generated_hit;
  function constant_z;
    input selector;
    begin
      casez (selector)
        1'bx: constant_z = 1'b1;
        default: constant_z = 1'b0;
      endcase
    end
  endfunction
  localparam FRONTEND_Z = constant_z(1'bz);
  generate
    if (FRONTEND_Z) begin : yes
      assign generated_hit = 1'b1;
    end else begin : no
      assign generated_hit = 1'b0;
    end
  endgenerate
  task check;
    input selector;
    input item;
    reg exact_hit, z_hit, x_hit;
    begin
      case (selector)
        item: exact_hit = 1'b1;
        default: exact_hit = 1'b0;
      endcase
      casez (selector)
        item: z_hit = 1'b1;
        default: z_hit = 1'b0;
      endcase
      casex (selector)
        item: x_hit = 1'b1;
        default: x_hit = 1'b0;
      endcase
      $display("pair=%b%b exact=%b z=%b x=%b", selector, item, exact_hit, z_hit, x_hit);
    end
  endtask
  task check_wide;
    input [128:0] selector;
    input [128:0] item;
    reg hit;
    begin
      casez (selector)
        item: hit = 1'b1;
        default: hit = 1'b0;
      endcase
      $display("wide=%b", hit);
    end
  endtask
  initial begin
    check(1'b0, 1'b0);
    check(1'b0, 1'b1);
    check(1'b0, 1'bx);
    check(1'b0, 1'bz);
    check(1'b1, 1'b0);
    check(1'b1, 1'b1);
    check(1'b1, 1'bx);
    check(1'b1, 1'bz);
    check(1'bx, 1'b0);
    check(1'bx, 1'b1);
    check(1'bx, 1'bx);
    check(1'bx, 1'bz);
    check(1'bz, 1'b0);
    check(1'bz, 1'b1);
    check(1'bz, 1'bx);
    check(1'bz, 1'bz);
    casez (1'bz)
      1'bx: constant_hit = 1'b1;
      default: constant_hit = 1'b0;
    endcase
    #1;
    $display("constant=%b frontend=%b generated=%b", constant_hit, FRONTEND_Z, generated_hit);
    check_wide({1'bz, 63'b0, 1'bz, 64'b0}, 129'b0);
    check_wide({1'bz, 63'b0, 1'bz, 64'b0}, 129'b1);
    $finish(0);
  end
endmodule

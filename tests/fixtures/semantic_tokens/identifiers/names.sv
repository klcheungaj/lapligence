// Names before their declaration, unknown names and names the frontend binds
// and then discards.
package names_pkg;
  typedef enum logic [1:0] {IDLE, BUSY} state_t;
  typedef struct packed {logic [3:0] hi; logic [3:0] lo;} pair_t;
  localparam int DEPTH = 4;
  localparam pair_t RESET = '{hi: 4'd1, lo: 4'd2};
  function automatic int twice(input int value);
    return value * 2;
  endfunction : twice
endpackage : names_pkg

module leaf #(parameter int WIDTH = 1, parameter int EXTRA = WIDTH) (
  input logic [WIDTH-1:0] data, output logic ready
);
  logic [WIDTH-1:0] held;
  assign ready = &data;
  assign held = data;
endmodule : leaf

module names_top import names_pkg::*; (input logic clk);
  assign before_wire = late;
  logic late;
  assign late = clk;

  state_t state;
  pair_t pair, pairs [2];
  typedef logic [(int'(RESET.lo) % 7):0] narrow_t;
  narrow_t narrow;
  logic [DEPTH-1:0] word;
  logic ready, done;

  (* keep, async_reg = "true", depth = DEPTH *) logic marked;
  leaf #(.WIDTH(DEPTH), .UNKNOWN(1)) u_leaf (.data(word), .ready(ready), .absent(done));
  ghost_module #(.WIDTH(DEPTH)) u_ghost (.data(word));
  leaf u_array [2] (.data(clk), .ready());
  defparam u_leaf.EXTRA = DEPTH;

  for (genvar i = 0; i < DEPTH; i++) begin : lanes
    logic [1:0] cell_bits;
    leaf u_lane (.data(clk), .ready());
  end : lanes
  if (DEPTH > 2) begin : taken
    logic inside_taken;
  end
  else begin : untaken
    logic inside_untaken;
    leaf u_untaken (.data(inside_untaken), .ready());
    assign inside_untaken = u_untaken.held;
  end

  always_ff @(posedge clk) begin : main
    (* full_case, style = DEPTH *) case (state)
      IDLE: state <= BUSY;
      default: state <= names_pkg::IDLE;
    endcase
    pair <= pair_t'{hi: 4'd3, lo: 4'd4};
    pairs <= '{pair_t: RESET};
    pair.hi <= pairs[DEPTH-3].lo;
    word <= (pairs[0].hi) + lanes[DEPTH-2].cell_bits[0] + u_array[DEPTH-4].held;
    word[0] <= lanes[1].u_lane.held && u_leaf.held[DEPTH-1];
    ready <= missing_name + done;
    ready <= missing_call(word, pair.lo);
    done <= pair.nothing;
    narrow <= narrow_t'(names_pkg::twice(.value(DEPTH)));
    if ((twice(1)) == $clog2(DEPTH)) disable main;
  end : main

  covergroup cover_state @(posedge clk);
    point_a: coverpoint state {bins idle = {IDLE};}
    point_b: coverpoint word;
    both: cross point_a, point_b;
  endgroup
  default clocking main_clock @(posedge clk);
    input ready;
  endclocking
endmodule

bind leaf checker_m u_checker (.seen(held[0]));
module checker_m (input logic seen);
endmodule

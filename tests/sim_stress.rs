//! End-to-end simulator stress tests: larger, realistic regression designs
//! exercised through the full Slang compile → codegen → CMake build → run
//! pipeline, asserting the exact stdout against hand-simulated traces.
//!
//! Designs:
//!   1. `stress_fifo_push_pop`      — DEPTH=4/DW=8 FIFO (head/tail pointers,
//!      full/empty flags, registered read) with a
//!      push/pop/overrun/underrun testbench.
//!   2. `stress_cpu_lite_datapath`  — 4-bit ALU + 2-reg register file + FSM
//!      controller executing a tiny program
//!      (load/add/sub/and/or/eq), synch reset.
//!   3. `stress_uart_shift_baud_ps` — 8-bit shift register + DIV-based baud
//!      generator transmitting 0xA5 LSB-first
//!      (start + 8 data + stop-ish), with a
//!      receiver-side reassembly in the testbench.
//!      `stress_uart_shift_baud` is the same
//!      design reading the shift bit through a
//!      bit-select instead of a part-select.
//!   4. `stress_wide_comb_tree`     — 64-bit adder chain + 4-way mux +
//!      casez priority encoder + reduction XOR,
//!      all combinational.
//!   5. `stress_gen_loop_instances` — parameterized module instantiated in a
//!      generate loop with per-iteration WIDTH,
//!      proving per-iteration parameter
//!      propagation and that instances inside
//!      generate scopes run.
//!
//! These tests temporarily change the process working directory, so each test
//! runs with the CWD pointed at a fresh temp dir (serialized through a mutex,
//! to avoid process-wide CWD races).

use std::sync::Mutex;

#[path = "support/sim.rs"]
mod sim_harness;

static CWD_LOCK: Mutex<()> = Mutex::new(());

/// Compile `sv`, codegen the model, build the simulator executable and run it,
/// returning the exact stdout.  Fails the test on any compile/codegen/cmake/run
/// error.  The caller must hold `CWD_LOCK`; the CWD is moved to a fresh
/// temp dir and restored afterwards.
fn run_sim(name: &str, sv: &str) -> String {
    sim_harness::run_sim(sv, "tb", name).expect("simulation should run")
}

// ── 1. FIFO ───────────────────────────────────────────────────────────────────

/// A DEPTH=4 / DW=8 synchronous FIFO with a registered read port, plus a
/// testbench that hand-drives a push/pop sequence: 4 pushes to fill, an
/// overrun push while full (must be ignored — proven by the first popped value
/// still being 0x11, not 0xEE), 2 pops, 1 push (wrapping head), then 3 more
/// pops to drain (popped values 0x11/0x22/0x33/0x44/0x55 in order) and a final
/// underrun pop while empty (must be ignored).
///
/// Hand-simulation.  All signals X at t=0.  Spawn order at t=0: input/output
/// links, tb `always #5`, tb `initial`, then the fifo `always_ff` (the child
/// instance's process spawns last; it registers its posedge-clk/negedge-rst_n
/// waiters after the initial has driven clk=0/rst_n=1, so the t=1 reset edge
/// X→0 on rst_n still counts as a negedge and resets the FIFO).
///
/// Clock posedges land at t = 5, 15, 25, … (clk=0 set at t=0, `always #5`
/// flips it).  Stimuli are written two ticks before the posedge that samples
/// them; displays happen a couple of ticks after a posedge so all NBAs and
/// port links have settled.
///
///   t=0   initial: clk=0, rst_n=1, push=0, pop=0, wdata=0; #1.
///   t=1   rst_n=0 → fifo.rst_n 1→0 (via link) → NEGEDGE → always_ff wakes:
///         reset NBAs (head<=0, tail<=0, full<=0, empty<=1, rdata<=0) commit:
///         head=0, tail=0, full=0, empty=1, rdata=00.  #3.
///   t=4   rst_n=1, push=1, wdata=0x11.  #10.
///   t=5   POSEDGE: push&&!full → mem[head=0]<=0x11, head<=1;
///         flags: full<=(0+1==tail=0)=0, empty<=0.
///         commit: mem[0]=0x11, head=1, full=0, empty=0.
///   t=14  push=1, wdata=0x22.  #10.
///   t=15  POSEDGE: mem[1]<=0x22, head<=2; full<=(1+1==0)=0.
///         commit: mem[1]=0x22, head=2.
///   t=24  push=1, wdata=0x33.  #10.
///   t=25  POSEDGE: mem[2]<=0x33, head<=3; full<=(2+1==0)=0.
///         commit: mem[2]=0x33, head=3.
///   t=34  push=1, wdata=0x44.  #10.
///   t=35  POSEDGE: mem[3]<=0x44, head<=3+1=4 → wraps to 0;
///         full<=(3+1==0)=0==0 → 1.  commit: mem[3]=0x44, head=0, full=1.
///   t=44  push=1, wdata=0xEE — an overrun attempt while full=1.  #2.
///   t=45  POSEDGE: push&&!full is FALSE (full=1), pop&&!empty is FALSE
///         (pop=0) → nothing recorded; the push is ignored (head stays 0,
///         mem[0] stays 0x11).  commit: no change.
///   t=46  $display("push-when-full: full=1 empty=0 head=0 tail=0 rdata=00").
///         #8.
///   t=54  push=0, pop=1.  #10.
///   t=55  POSEDGE: pop&&!empty → rdata<=mem[tail=0]=0x11 (the OLD value —
///         proves the overrun push at t=45 was ignored), tail<=1;
///         empty<=(0+1==head=0)=0, full<=0.  commit: rdata=11, tail=1, full=0.
///   t=64  pop=1.  #10.
///   t=65  POSEDGE: rdata<=mem[1]=0x22, tail<=2; empty<=(1+1==0)=0.
///         commit: rdata=22, tail=2.
///   t=74  pop=0, push=1, wdata=0x55.  #10.
///   t=75  POSEDGE: push&&!full → mem[head=0]<=0x55 (wraps over the drained
///         slot), head<=1; full<=(0+1==tail=2)=0.  commit: mem[0]=0x55, head=1.
///   t=84  push=0.  #3.
///   t=87  $display("full=0 empty=0 head=1 tail=2 rdata=22").
///         pop=1.  #7.
///   t=94  pop=1.  #10.
///   t=95  POSEDGE: rdata<=mem[2]=0x33, tail<=3; empty<=(2+1==head=1)=0.
///         commit: rdata=33, tail=3.
///   t=104 pop=1.  #10.
///   t=105 POSEDGE: rdata<=mem[3]=0x44, tail<=0; empty<=(3+1==head=1)=0.
///         commit: rdata=44, tail=0.
///   t=114 pop=1.  #10.
///   t=115 POSEDGE: rdata<=mem[0]=0x55, tail<=1;
///         empty<=(0+1==head=1)=1.  commit: rdata=55, tail=1, empty=1.
///   t=124 pop=1.  #2.
///   t=125 POSEDGE: pop&&!empty is FALSE (empty=1) → ignored; rdata stays 55.
///   t=126 $display("full=0 empty=1 head=1 tail=1 rdata=55").  #10.
///   t=135 POSEDGE: pop=1 but empty=1 → ignored again; no state change.
///   t=136 $display("pop-when-empty: empty=1 head=1 tail=1 rdata=55");
///         $finish.
///
/// Expected stdout (exactly):
///   push-when-full: full=1 empty=0 head=0 tail=0 rdata=00
///   full=0 empty=0 head=1 tail=2 rdata=22
///   full=0 empty=1 head=1 tail=1 rdata=55
///   pop-when-empty: empty=1 head=1 tail=1 rdata=55
#[test]
fn stress_fifo_push_pop() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module fifo #(parameter DEPTH = 4, parameter DW = 8) (
    input  logic        clk,
    input  logic        rst_n,
    input  logic        push,
    input  logic        pop,
    input  logic [DW-1:0] wdata,
    output logic [DW-1:0] rdata,
    output logic        full,
    output logic        empty,
    output logic [1:0]  head,
    output logic [1:0]  tail
);
    reg [DW-1:0] mem [0:DEPTH-1];

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            head  <= 2'd0;
            tail  <= 2'd0;
            full  <= 1'b0;
            empty <= 1'b1;
            rdata <= 8'h00;
        end else begin
            if (push && !full) begin
                mem[head] <= wdata;
                head <= head + 2'd1;
            end
            if (pop && !empty) begin
                rdata <= mem[tail];
                tail <= tail + 2'd1;
            end
            if (push && !full) begin
                full  <= (head + 2'd1 == tail);
                empty <= 1'b0;
            end else if (pop && !empty) begin
                empty <= (tail + 2'd1 == head);
                full  <= 1'b0;
            end
        end
    end
endmodule

module tb;
    reg clk, rst_n, push, pop;
    reg [7:0] wdata;
    wire [7:0] rdata;
    wire full, empty;
    wire [1:0] head, tail;

    fifo #(.DEPTH(4), .DW(8)) u_fifo(
        .clk(clk), .rst_n(rst_n), .push(push), .pop(pop), .wdata(wdata),
        .rdata(rdata), .full(full), .empty(empty), .head(head), .tail(tail)
    );

    always #5 clk = ~clk;

    initial begin
        clk = 0; rst_n = 1; push = 0; pop = 0; wdata = 8'h00;
        #1 rst_n = 0;
        #3 rst_n = 1; push = 1; wdata = 8'h11;
        #10 push = 1; wdata = 8'h22;
        #10 push = 1; wdata = 8'h33;
        #10 push = 1; wdata = 8'h44;
        #10 push = 1; wdata = 8'hEE;
        #2 $display("push-when-full: full=%b empty=%b head=%0d tail=%0d rdata=%h", full, empty, head, tail, rdata);
        #8 push = 0; pop = 1;
        #10 pop = 1;
        #10 pop = 0; push = 1; wdata = 8'h55;
        #10 push = 0;
        #3 $display("full=%b empty=%b head=%0d tail=%0d rdata=%h", full, empty, head, tail, rdata);
        pop = 1;
        #7 pop = 1;
        #10 pop = 1;
        #10 pop = 1;
        #10 pop = 1;
        #2 $display("full=%b empty=%b head=%0d tail=%0d rdata=%h", full, empty, head, tail, rdata);
        #10 $display("pop-when-empty: empty=%b head=%0d tail=%0d rdata=%h", empty, head, tail, rdata);
        $finish;
    end
endmodule
"#;
    assert_eq!(
        run_sim("fifo", sv),
        "push-when-full: full=1 empty=0 head=0 tail=0 rdata=00\n\
         full=0 empty=0 head=1 tail=2 rdata=22\n\
         full=0 empty=1 head=1 tail=1 rdata=55\n\
         pop-when-empty: empty=1 head=1 tail=1 rdata=55\n"
    );
}

// ── 2. CPU-lite datapath ─────────────────────────────────────────────────────

/// A 4-bit ALU (add/sub/and/or + `eq` flag, 2-bit opcode) + a 2-entry
/// register file + a synchronous-reset FSM controller executing a tiny program
/// and driving the ALU through all four ops.  The register file reads are
/// copied to the `ra`/`rb` output ports every cycle (registered copies, one
/// cycle behind the file itself).
///
/// Program (state machine, `state` = 4 bits):
///   0 idle (start → 1)    1 reg0 <= 3       2 reg1 <= 5
///   3 latch {reg0,reg1,+} 4 reg0 <= y (8)   5 latch {reg0,reg1,-}
///   6 reg1 <= y (3)       7 latch {reg0,reg1,&}  8 reg0 <= y (0)
///   9 latch {reg0,reg1,|} 10 reg1 <= y (3)  11 latch {reg0,reg0,|}
///   12 done <= 1 → idle
///
/// Hand-simulation.  Posedges at t = 5, 15, 25, …; stimuli at t ≡ 0/4
/// (mod 10), displays at t ≡ 7 (mod 10).  `alu_a`/`alu_b`/`alu_op` start X and
/// are only written in the latch states, so `alu_y`/`eq` read X until the first
/// latch commits.  The ALU's `y` is a `case` with `default: y = a | b`, so an X
/// opcode falls through to `default` (y = X|X = X), and `eq = (a == b)` is X
/// when either operand is X (4-state `==`).
///
///   t=0   spawn: tb.always#5, tb.initial, cpu_lite.always_ff, u_alu.comb.
///         initial: clk=0, rst_n=1, start=0; #1.
///   t=1   rst_n=0 → negedge → reset NBAs commit: state=0, reg0=0, reg1=0,
///         ra=0, rb=0, done=0.  #3.
///   t=4   rst_n=1.  #6.
///   t=10  start=1.  #10.
///   t=15  POSEDGE state0,start → state<=1; ra<=0, rb<=0.  commit: state=1,
///         ra=0, rb=0.
///   t=20  start=0.  #7.
///   t=25  POSEDGE state1 → reg0<=3, state<=2; ra<=reg0=0, rb<=reg1=0.
///         commit: reg0=3, state=2, ra=0, rb=0.
///   t=27  $display("ra=0 rb=0 alu_y=x eq=x done=0").
///   t=35  POSEDGE state2 → reg1<=5, state<=3; ra<=reg0=3, rb<=reg1=0.
///         commit: reg1=5, state=3, ra=3, rb=0.
///   t=37  $display("ra=3 rb=0 alu_y=x eq=x done=0").
///   t=45  POSEDGE state3 → alu_a<=3, alu_b<=5, alu_op<=0, state<=4;
///         ra<=3, rb<=5.  commit: alu_a=3, alu_b=5, alu_op=0 → y=3+5=8,
///         eq=0; ra=3, rb=5, state=4.
///   t=47  $display("ra=3 rb=5 alu_y=8 eq=0 done=0").
///   t=55  POSEDGE state4 → reg0<=y=8, state<=5; ra<=reg0=3, rb<=5.
///         commit: reg0=8, state=5, ra=3, rb=5.
///   t=57  $display("ra=3 rb=5 alu_y=8 eq=0 done=0").
///   t=65  POSEDGE state5 → alu_a<=8, alu_b<=5, alu_op<=1, state<=6; ra<=8.
///         commit: y=8-5=3, ra=8, rb=5, state=6.
///   t=67  $display("ra=8 rb=5 alu_y=3 eq=0 done=0").
///   t=75  POSEDGE state6 → reg1<=y=3, state<=7; ra<=8, rb<=reg1=5.
///         commit: reg1=3, state=7, ra=8, rb=5.
///   t=77  $display("ra=8 rb=5 alu_y=3 eq=0 done=0").
///   t=85  POSEDGE state7 → alu_a<=8, alu_b<=3, alu_op<=2, state<=8; rb<=3.
///         commit: y=8&3=0, rb=3, state=8.
///   t=87  $display("ra=8 rb=3 alu_y=0 eq=0 done=0").
///   t=95  POSEDGE state8 → reg0<=y=0, state<=9; ra<=reg0=8, rb<=3.
///         commit: reg0=0, state=9, ra=8, rb=3.
///   t=97  $display("ra=8 rb=3 alu_y=0 eq=0 done=0").
///   t=105 POSEDGE state9 → alu_a<=0, alu_b<=3, alu_op<=3, state<=10; ra<=0.
///         commit: y=0|3=3, eq=0, ra=0, state=10.
///   t=107 $display("ra=0 rb=3 alu_y=3 eq=0 done=0").
///   t=115 POSEDGE state10 → reg1<=y=3, state<=11; ra<=0, rb<=reg1=3.
///         commit: state=11, ra=0, rb=3 (reg1 stays 3).
///   t=117 $display("ra=0 rb=3 alu_y=3 eq=0 done=0").
///   t=125 POSEDGE state11 → alu_a<=reg0=0, alu_b<=reg0=0, alu_op<=3, state<=12.
///         commit: y=0|0=0, eq=(0==0)=1, state=12.
///   t=127 $display("ra=0 rb=3 alu_y=0 eq=1 done=0").
///   t=135 POSEDGE state12 → done<=1, state<=0; ra<=0, rb<=3.
///         commit: done=1, state=0, ra=0, rb=3.
///   t=137 $display("ra=0 rb=3 alu_y=0 eq=1 done=1").
///   t=145 POSEDGE state0, start=0 → nothing; ra<=0, rb<=3.
///   t=147 $display("ra=0 rb=3 alu_y=0 eq=1 done=1"); $finish.
///
/// Expected stdout (exactly):
///   t=27 ra=0 rb=0 alu_y=x eq=x done=0
///   t=37 ra=3 rb=0 alu_y=x eq=x done=0
///   t=47 ra=3 rb=5 alu_y=8 eq=0 done=0
///   t=57 ra=3 rb=5 alu_y=8 eq=0 done=0
///   t=67 ra=8 rb=5 alu_y=3 eq=0 done=0
///   t=77 ra=8 rb=5 alu_y=3 eq=0 done=0
///   t=87 ra=8 rb=3 alu_y=0 eq=0 done=0
///   t=97 ra=8 rb=3 alu_y=0 eq=0 done=0
///   t=107 ra=0 rb=3 alu_y=3 eq=0 done=0
///   t=117 ra=0 rb=3 alu_y=3 eq=0 done=0
///   t=127 ra=0 rb=3 alu_y=0 eq=1 done=0
///   t=137 ra=0 rb=3 alu_y=0 eq=1 done=1
///   t=147 ra=0 rb=3 alu_y=0 eq=1 done=1
#[test]
fn stress_cpu_lite_datapath() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module alu (
    input  logic [1:0] op,
    input  logic [3:0] a,
    input  logic [3:0] b,
    output logic [3:0] y,
    output logic eq
);
    always_comb begin
        eq = (a == b);
        case (op)
            2'd0: y = a + b;
            2'd1: y = a - b;
            2'd2: y = a & b;
            default: y = a | b;
        endcase
    end
endmodule

module cpu_lite (
    input  logic clk, rst_n, start,
    output logic [3:0] ra, rb,
    output logic [3:0] alu_y,
    output logic eq,
    output logic done
);
    reg [3:0] state;
    reg [3:0] regfile [0:1];
    reg [3:0] alu_a, alu_b;
    reg [1:0] alu_op;
    wire [3:0] y;
    wire eq_w;

    alu u_alu(.op(alu_op), .a(alu_a), .b(alu_b), .y(y), .eq(eq_w));
    assign alu_y = y;
    assign eq = eq_w;

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            state <= 4'd0;
            regfile[0] <= 4'd0;
            regfile[1] <= 4'd0;
            ra <= 4'd0;
            rb <= 4'd0;
            done <= 1'b0;
        end else begin
            ra <= regfile[0];
            rb <= regfile[1];
            case (state)
                4'd0: if (start) state <= 4'd1;
                4'd1: begin regfile[0] <= 4'd3; state <= 4'd2; end
                4'd2: begin regfile[1] <= 4'd5; state <= 4'd3; end
                4'd3: begin alu_a <= regfile[0]; alu_b <= regfile[1]; alu_op <= 2'd0; state <= 4'd4; end
                4'd4: begin regfile[0] <= y; state <= 4'd5; end
                4'd5: begin alu_a <= regfile[0]; alu_b <= regfile[1]; alu_op <= 2'd1; state <= 4'd6; end
                4'd6: begin regfile[1] <= y; state <= 4'd7; end
                4'd7: begin alu_a <= regfile[0]; alu_b <= regfile[1]; alu_op <= 2'd2; state <= 4'd8; end
                4'd8: begin regfile[0] <= y; state <= 4'd9; end
                4'd9: begin alu_a <= regfile[0]; alu_b <= regfile[1]; alu_op <= 2'd3; state <= 4'd10; end
                4'd10: begin regfile[1] <= y; state <= 4'd11; end
                4'd11: begin alu_a <= regfile[0]; alu_b <= regfile[0]; alu_op <= 2'd3; state <= 4'd12; end
                4'd12: begin done <= 1'b1; state <= 4'd0; end
                default: state <= 4'd0;
            endcase
        end
    end
endmodule

module tb;
    reg clk, rst_n, start;
    wire [3:0] ra, rb, alu_y;
    wire eq, done;

    cpu_lite u_cpu(.clk(clk), .rst_n(rst_n), .start(start),
                   .ra(ra), .rb(rb), .alu_y(alu_y), .eq(eq), .done(done));

    always #5 clk = ~clk;

    initial begin
        clk = 0; rst_n = 1; start = 0;
        #1 rst_n = 0;
        #3 rst_n = 1;
        #6 start = 1;
        #10 start = 0;
        #7 $display("t=27 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=37 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=47 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=57 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=67 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=77 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=87 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=97 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=107 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=117 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=127 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=137 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        #10 $display("t=147 ra=%0d rb=%0d alu_y=%0d eq=%b done=%b", ra, rb, alu_y, eq, done);
        $finish;
    end
endmodule
"#;
    assert_eq!(
        run_sim("cpu", sv),
        "t=27 ra=0 rb=0 alu_y=x eq=x done=0\n\
         t=37 ra=3 rb=0 alu_y=x eq=x done=0\n\
         t=47 ra=3 rb=5 alu_y=8 eq=0 done=0\n\
         t=57 ra=3 rb=5 alu_y=8 eq=0 done=0\n\
         t=67 ra=8 rb=5 alu_y=3 eq=0 done=0\n\
         t=77 ra=8 rb=5 alu_y=3 eq=0 done=0\n\
         t=87 ra=8 rb=3 alu_y=0 eq=0 done=0\n\
         t=97 ra=8 rb=3 alu_y=0 eq=0 done=0\n\
         t=107 ra=0 rb=3 alu_y=3 eq=0 done=0\n\
         t=117 ra=0 rb=3 alu_y=3 eq=0 done=0\n\
         t=127 ra=0 rb=3 alu_y=0 eq=1 done=0\n\
         t=137 ra=0 rb=3 alu_y=0 eq=1 done=1\n\
         t=147 ra=0 rb=3 alu_y=0 eq=1 done=1\n"
    );
}

// ── 3. UART-ish shift register / baud generator ──────────────────────────────

/// A UART-style transmitter: an 8-bit shift register whose bit times come from
/// a DIV-based divider counter (`baud_cnt` 0..DIV-1; a "baud tick" every DIV
/// clocks).  On `tx_start` (while idle) it drives a start bit (0), loads
/// `tx_data`, then on every baud tick shifts the LSB out (filling with 1s, so
/// the line returns to idle-high); `tx_busy` deasserts after the 8th data bit.
///
/// Transmits 0xA5 (1010_0101) LSB-first with DIV=4 (one baud period = 4 clocks
/// = 40 ticks), then restarts a second byte to prove the FSM re-enters.
///
/// This variant reads the shifted-out bit through a 1-bit PART-select
/// (`shreg[0:0]`) instead of a bit-select (`shreg[0]`); the two designs are
/// otherwise identical (`stress_uart_shift_baud` covers the bit-select path).
///
/// Hand-simulation.  Posedges at t = 5, 15, 25, …; a baud tick fires on the
/// posedge where `baud_cnt == 3`, i.e. at t = 55, 95, 135, 175, 215, 255, 295,
/// 335 (8 data bits) — the start bit is committed at t=15 (the posedge that
/// samples `tx_start`), and the 8th tick (t=335) deasserts `busy`.
///
///   t=0   initial: clk=0, rst_n=1, tx_start=0, tx_data=0xA5, rx_buf=0; #1.
///   t=1   rst_n=0 → negedge → reset NBAs commit: baud_cnt=0, bit_cnt=0,
///         shreg=0, tx_line=1 (idle high), busy=0.  #3.
///   t=4   rst_n=1.  #8.
///   t=12  $display("idle: tx=1 busy=0").  #2.
///   t=14  tx_start=1.  #2.
///   t=15  POSEDGE tx_start&&!busy → busy<=1, bit_cnt<=0, baud_cnt<=0,
///         shreg<=0xA5, tx_line<=0 (start bit).  commit: busy=1, tx=0.
///   t=16  tx_start=0.  #1.
///   t=17  $display("start: tx=0 busy=1").
///         baud_cnt counts 0→1 (t=25), 1→2 (t=35), 2→3 (t=45)…
///   t=55  TICK: tx_line<=shreg[0:0]=1 (bit0 of 0xA5), shreg<=0xD2, bit_cnt<=1.
///         commit: tx=1, busy=1.
///   t=57  rx_buf<={rx_buf[6:0],tx}=0x01; $display("bit0: tx=1 busy=1").
///   t=95  TICK: tx_line<=0 (bit1), shreg<=0xE9, bit_cnt<=2.
///   t=97  rx_buf=0x02; $display("bit1: tx=0 busy=1").
///   t=135 TICK: tx_line<=1 (bit2), shreg<=0xF4, bit_cnt<=3.
///   t=137 rx_buf=0x05; $display("bit2: tx=1 busy=1").
///   t=175 TICK: tx_line<=0 (bit3), shreg<=0xFA, bit_cnt<=4.
///   t=177 rx_buf=0x0A; $display("bit3: tx=0 busy=1").
///   t=215 TICK: tx_line<=0 (bit4), shreg<=0xFD, bit_cnt<=5.
///   t=217 rx_buf=0x14; $display("bit4: tx=0 busy=1").
///   t=255 TICK: tx_line<=1 (bit5), shreg<=0xFE, bit_cnt<=6.
///   t=257 rx_buf=0x29; $display("bit5: tx=1 busy=1").
///   t=295 TICK: tx_line<=0 (bit6), shreg<=0xFF, bit_cnt<=7.
///   t=297 rx_buf=0x52; $display("bit6: tx=0 busy=1").
///   t=335 TICK, bit_cnt==7: tx_line<=shreg[0:0]=1 (bit7), busy<=0, bit_cnt<=0.
///         commit: tx=1, busy=0.
///   t=337 rx_buf=0xA5; $display("bit7: tx=1 busy=0").
///   t=347 $display("received=a5 busy=0").  #7.
///   t=354 tx_start=1.  #10.
///   t=355 POSEDGE tx_start&&!busy → busy<=1, tx_line<=0 (second start bit).
///   t=364 tx_start=0.  #3.
///   t=367 $display("restart: tx=0 busy=1"); $finish.
///
/// Expected stdout (exactly):
///   t=12 idle: tx=1 busy=0
///   t=17 start: tx=0 busy=1
///   t=57 bit0: tx=1 busy=1
///   t=97 bit1: tx=0 busy=1
///   t=137 bit2: tx=1 busy=1
///   t=177 bit3: tx=0 busy=1
///   t=217 bit4: tx=0 busy=1
///   t=257 bit5: tx=1 busy=1
///   t=297 bit6: tx=0 busy=1
///   t=337 bit7: tx=1 busy=0
///   t=347 received=a5 busy=0
///   t=367 restart: tx=0 busy=1
#[test]
fn stress_uart_shift_baud_ps() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module uart_tx #(parameter DIV = 4) (
    input  logic clk,
    input  logic rst_n,
    input  logic tx_start,
    input  logic [7:0] tx_data,
    output logic tx,
    output logic tx_busy
);
    reg [3:0] baud_cnt;
    reg [2:0] bit_cnt;
    reg [7:0] shreg;
    reg tx_line;
    reg busy;

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            baud_cnt <= 4'd0;
            bit_cnt  <= 3'd0;
            shreg    <= 8'd0;
            tx_line  <= 1'b1;
            busy     <= 1'b0;
        end else if (tx_start && !busy) begin
            busy     <= 1'b1;
            bit_cnt  <= 3'd0;
            baud_cnt <= 4'd0;
            shreg    <= tx_data;
            tx_line  <= 1'b0;              // start bit
        end else if (busy) begin
            if (baud_cnt == DIV - 1) begin // baud tick
                baud_cnt <= 4'd0;
                tx_line  <= shreg[0:0];    // 1-bit part-select (works)
                shreg    <= {1'b1, shreg[7:1]};
                if (bit_cnt == 3'd7) begin
                    busy    <= 1'b0;
                    bit_cnt <= 3'd0;
                end else begin
                    bit_cnt <= bit_cnt + 3'd1;
                end
            end else begin
                baud_cnt <= baud_cnt + 4'd1;
            end
        end else begin
            baud_cnt <= 4'd0;
        end
    end

    assign tx = tx_line;
    assign tx_busy = busy;
endmodule

module tb;
    reg clk, rst_n, tx_start;
    reg [7:0] tx_data;
    wire tx, tx_busy;
    reg [7:0] rx_buf;

    uart_tx #(.DIV(4)) u_tx(.clk(clk), .rst_n(rst_n), .tx_start(tx_start),
                            .tx_data(tx_data), .tx(tx), .tx_busy(tx_busy));

    always #5 clk = ~clk;

    initial begin
        clk = 0; rst_n = 1; tx_start = 0; tx_data = 8'hA5; rx_buf = 8'h00;
        #1 rst_n = 0;
        #3 rst_n = 1;
        #8 $display("t=12 idle: tx=%b busy=%b", tx, tx_busy);
        #2 tx_start = 1;
        #2 tx_start = 0;
        #1 $display("t=17 start: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=57 bit0: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=97 bit1: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=137 bit2: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=177 bit3: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=217 bit4: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=257 bit5: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=297 bit6: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=337 bit7: tx=%b busy=%b", tx, tx_busy);
        #10 $display("t=347 received=%h busy=%b", rx_buf, tx_busy);
        #7 tx_start = 1;
        #10 tx_start = 0;
        #3 $display("t=367 restart: tx=%b busy=%b", tx, tx_busy);
        $finish;
    end
endmodule
"#;
    assert_eq!(
        run_sim("uart", sv),
        "t=12 idle: tx=1 busy=0\n\
         t=17 start: tx=0 busy=1\n\
         t=57 bit0: tx=1 busy=1\n\
         t=97 bit1: tx=0 busy=1\n\
         t=137 bit2: tx=1 busy=1\n\
         t=177 bit3: tx=0 busy=1\n\
         t=217 bit4: tx=0 busy=1\n\
         t=257 bit5: tx=1 busy=1\n\
         t=297 bit6: tx=0 busy=1\n\
         t=337 bit7: tx=1 busy=0\n\
         t=347 received=a5 busy=0\n\
         t=367 restart: tx=0 busy=1\n"
    );
}

/// The SAME UART design with the shifted-out bit read through a plain
/// bit-select (`shreg[0]`) instead of a part-select.  The hand-simulated
/// trace is identical to `stress_uart_shift_baud_ps`:
///   t=12 idle: tx=1 busy=0
///   t=17 start: tx=0 busy=1
///   t=57 bit0: tx=1 busy=1
///   … (identical to the part-select variant)
#[test]
fn stress_uart_shift_baud() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module uart_tx #(parameter DIV = 4) (
    input  logic clk,
    input  logic rst_n,
    input  logic tx_start,
    input  logic [7:0] tx_data,
    output logic tx,
    output logic tx_busy
);
    reg [3:0] baud_cnt;
    reg [2:0] bit_cnt;
    reg [7:0] shreg;
    reg tx_line;
    reg busy;

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            baud_cnt <= 4'd0;
            bit_cnt  <= 3'd0;
            shreg    <= 8'd0;
            tx_line  <= 1'b1;
            busy     <= 1'b0;
        end else if (tx_start && !busy) begin
            busy     <= 1'b1;
            bit_cnt  <= 3'd0;
            baud_cnt <= 4'd0;
            shreg    <= tx_data;
            tx_line  <= 1'b0;              // start bit
        end else if (busy) begin
            if (baud_cnt == DIV - 1) begin // baud tick
                baud_cnt <= 4'd0;
                tx_line  <= shreg[0];      // bit-select
                shreg    <= {1'b1, shreg[7:1]};
                if (bit_cnt == 3'd7) begin
                    busy    <= 1'b0;
                    bit_cnt <= 3'd0;
                end else begin
                    bit_cnt <= bit_cnt + 3'd1;
                end
            end else begin
                baud_cnt <= baud_cnt + 4'd1;
            end
        end else begin
            baud_cnt <= 4'd0;
        end
    end

    assign tx = tx_line;
    assign tx_busy = busy;
endmodule

module tb;
    reg clk, rst_n, tx_start;
    reg [7:0] tx_data;
    wire tx, tx_busy;
    reg [7:0] rx_buf;

    uart_tx #(.DIV(4)) u_tx(.clk(clk), .rst_n(rst_n), .tx_start(tx_start),
                            .tx_data(tx_data), .tx(tx), .tx_busy(tx_busy));

    always #5 clk = ~clk;

    initial begin
        clk = 0; rst_n = 1; tx_start = 0; tx_data = 8'hA5; rx_buf = 8'h00;
        #1 rst_n = 0;
        #3 rst_n = 1;
        #8 $display("t=12 idle: tx=%b busy=%b", tx, tx_busy);
        #2 tx_start = 1;
        #2 tx_start = 0;
        #1 $display("t=17 start: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=57 bit0: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=97 bit1: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=137 bit2: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=177 bit3: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=217 bit4: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=257 bit5: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=297 bit6: tx=%b busy=%b", tx, tx_busy);
        #40 rx_buf = {rx_buf[6:0], tx}; $display("t=337 bit7: tx=%b busy=%b", tx, tx_busy);
        #10 $display("t=347 received=%h busy=%b", rx_buf, tx_busy);
        #7 tx_start = 1;
        #10 tx_start = 0;
        #3 $display("t=367 restart: tx=%b busy=%b", tx, tx_busy);
        $finish;
    end
endmodule
"#;
    let stdout = run_sim("uart_bs", sv);
    assert_eq!(
        stdout,
        "t=12 idle: tx=1 busy=0\n\
         t=17 start: tx=0 busy=1\n\
         t=57 bit0: tx=1 busy=1\n\
         t=97 bit1: tx=0 busy=1\n\
         t=137 bit2: tx=1 busy=1\n\
         t=177 bit3: tx=0 busy=1\n\
         t=217 bit4: tx=0 busy=1\n\
         t=257 bit5: tx=1 busy=1\n\
         t=297 bit6: tx=0 busy=1\n\
         t=337 bit7: tx=1 busy=0\n\
         t=347 received=a5 busy=0\n\
         t=367 restart: tx=0 busy=1\n"
    );
}

// ── 4. Wide combinational tree ───────────────────────────────────────────────

/// A 64-bit combinational stress design: a two-stage adder chain (sum1 = a+b,
/// sum2 = sum1+c), a 4-way priority mux over {a, b, sum2, d}, a 64-bit
/// reduction-XOR, and an 8-input casez priority encoder.  Everything is
/// combinational (`assign` + `always_comb`), so every display happens one tick
/// after the stimulus that settled it.
///
/// Hand-simulation (all combs evaluate with X at t=0, then re-run when their
/// read set changes; the initial's blocking writes wake them within t=0):
///
///   t=0   combs evaluate with X inputs: sum1=X, sum2=X, parity=X; the mux
///         chain `sel==0 ? a : sel==1 ? b : sel==2 ? sum2 : d` with an X
///         select makes every `sel==k` comparison X (not true), so each
///         ternary falls to its else → muxed=d=X; casez with an X selector
///         matches no item (each item has a known bit the X selector can't
///         match) → enc=0.  initial: a=1, b=2, c=3, d=4, sel=2, prio=0x20 →
///         combs re-run: sum1=1+2=3, sum2=3+3=6, muxed=(sel==2)→sum2=6,
///         parity=^6=^0b110=0, enc=0b00100000 → item 8'b001????? → 5.  #1.
///   t=1   $display("t=1 sum1=3 sum2=6 muxed=6 parity=0 enc=5").
///         sel=3, a=2^64-1: sum1=(2^64-1)+2 = 2^64+1 → 64-bit wrap → 1,
///         sum2=1+3=4, muxed=(sel==3)→d=4, parity=^4=1.  #1.
///   t=2   $display("t=2 sum1=1 sum2=4 muxed=4 parity=1 enc=5").
///         sel=0 → muxed=a=2^64-1.  #1.
///   t=3   $display("t=3 sum1=1 sum2=4 muxed=18446744073709551615 parity=1").
///         prio=0x02 → item 8'b0000001? → enc=1.  #1.
///   t=4   $display("t=4 enc=1").  prio=0x80 → item 8'b1??????? → enc=7.  #1.
///   t=5   $display("t=5 enc=7").  prio=0 → no item matches (item 8'b00000001
///         needs bit0=1) → default → enc=0.  #1.
///   t=6   $display("t=6 enc=0"); $finish.
///
/// Expected stdout (exactly):
///   t=1 sum1=3 sum2=6 muxed=6 parity=0 enc=5
///   t=2 sum1=1 sum2=4 muxed=4 parity=1 enc=5
///   t=3 sum1=1 sum2=4 muxed=18446744073709551615 parity=1
///   t=4 enc=1
///   t=5 enc=7
///   t=6 enc=0
#[test]
fn stress_wide_comb_tree() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg [63:0] a, b, c, d;
    reg [1:0] sel;
    reg [7:0] prio;
    reg [2:0] enc;
    wire [63:0] sum1, sum2, muxed;
    wire parity;

    assign sum1  = a + b;
    assign sum2  = sum1 + c;             // adder chain: reads sum1
    assign muxed = (sel == 2'd0) ? a :
                   (sel == 2'd1) ? b :
                   (sel == 2'd2) ? sum2 : d;
    assign parity = ^sum2;               // 64-bit reduction XOR

    always_comb begin
        casez (prio)
            8'b1???????: enc = 3'd7;
            8'b01??????: enc = 3'd6;
            8'b001?????: enc = 3'd5;
            8'b0001????: enc = 3'd4;
            8'b00001???: enc = 3'd3;
            8'b000001??: enc = 3'd2;
            8'b0000001?: enc = 3'd1;
            8'b00000001: enc = 3'd0;
            default: enc = 3'd0;
        endcase
    end

    initial begin
        a = 64'd1; b = 64'd2; c = 64'd3; d = 64'd4;
        sel = 2'd2; prio = 8'b0010_0000;
        #1 $display("t=1 sum1=%0d sum2=%0d muxed=%0d parity=%b enc=%0d", sum1, sum2, muxed, parity, enc);
        sel = 2'd3; a = 64'hFFFF_FFFF_FFFF_FFFF;
        #1 $display("t=2 sum1=%0d sum2=%0d muxed=%0d parity=%b enc=%0d", sum1, sum2, muxed, parity, enc);
        sel = 2'd0;
        #1 $display("t=3 sum1=%0d sum2=%0d muxed=%0d parity=%b", sum1, sum2, muxed, parity);
        prio = 8'b0000_0010;
        #1 $display("t=4 enc=%0d", enc);
        prio = 8'b1000_0000;
        #1 $display("t=5 enc=%0d", enc);
        prio = 8'b0000_0000;
        #1 $display("t=6 enc=%0d", enc);
        $finish;
    end
endmodule
"#;
    assert_eq!(
        run_sim("comb", sv),
        "t=1 sum1=3 sum2=6 muxed=6 parity=0 enc=5\n\
         t=2 sum1=1 sum2=4 muxed=4 parity=1 enc=5\n\
         t=3 sum1=1 sum2=4 muxed=18446744073709551615 parity=1\n\
         t=4 enc=1\n\
         t=5 enc=7\n\
         t=6 enc=0\n"
    );
}

// ── 5. Multi-instance generate ────────────────────────────────────────────────

/// A parameterized counter module instantiated four times in a generate loop,
/// each with a different WIDTH, resetting to its own WIDTH and counting up —
/// proving per-iteration parameter propagation AND that the generated instances
/// (their signals, processes and port links) exist and run.
///
/// Hand-simulation:
///   t=0   spawn: links, tb.always#5, tb.initial, then the four gen-scope
///         instances' always_ff processes.  initial: clk=0, rst_n=1; #1.
///   t=1   rst_n=0 → negedge → every counter resets cnt<=WIDTH:
///         cnts = 4, 5, 6, 7.  #3.
///   t=4   rst_n=1.  #8.
///   t=5   POSEDGE: cnt<=cnt+1 → cnts = 5, 6, 7, 8.
///   t=12  $display("t=12 cnts=5 6 7 8").  #10.
///   t=15  POSEDGE: cnts = 6, 7, 8, 9.
///   t=22  $display("t=22 cnts=6 7 8 9").  #10.
///   t=25  POSEDGE: cnts = 7, 8, 9, 10.
///   t=32  $display("t=32 cnts=7 8 9 10"); $finish.
///
/// Expected stdout (exactly):
///   t=12 cnts=5 6 7 8
///   t=22 cnts=6 7 8 9
///   t=32 cnts=7 8 9 10
#[test]
fn stress_gen_loop_instances() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module gen_counter #(parameter WIDTH = 4) (
    input logic clk, input logic rst_n,
    output logic [WIDTH-1:0] cnt
);
    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) cnt <= WIDTH;
        else cnt <= cnt + 1;
    end
endmodule

module tb;
    reg clk, rst_n;
    wire [7:0] cnts [0:3];
    genvar i;
    for (i = 0; i < 4; i = i + 1) begin : g
        gen_counter #(.WIDTH(i + 4)) u(.clk(clk), .rst_n(rst_n), .cnt(cnts[i]));
    end

    always #5 clk = ~clk;

    initial begin
        clk = 0; rst_n = 1;
        #1 rst_n = 0;
        #3 rst_n = 1;
        #8 $display("t=12 cnts=%0d %0d %0d %0d", cnts[0], cnts[1], cnts[2], cnts[3]);
        #10 $display("t=22 cnts=%0d %0d %0d %0d", cnts[0], cnts[1], cnts[2], cnts[3]);
        #10 $display("t=32 cnts=%0d %0d %0d %0d", cnts[0], cnts[1], cnts[2], cnts[3]);
        $finish;
    end
endmodule
"#;
    let stdout = run_sim("genloop", sv);
    assert_eq!(
        stdout,
        "t=12 cnts=5 6 7 8\nt=22 cnts=6 7 8 9\nt=32 cnts=7 8 9 10\n"
    );
}

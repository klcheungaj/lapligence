// Decision A19-D48: nets of a declared data type. A net declared with a
// packed structure type is a vpiStructNet, with an enum type a vpiEnumNet,
// with integer a vpiIntegerNet, with time a vpiTimeNet, and with a packed
// array of structures or enums a vpiPackedArrayNet; a logic net, one with
// several packed dimensions included, is a vpiNet, and so is a union net
// (1800-2009 defines no union net type). An element of an array of such
// nets has the same type. vpi_iterate(vpiNet, scope) returns nets of every
// type. vpiPackedArrayMember is FALSE for these nets. Range iteration is an
// error on a structure net and an enum net. Members of a structure net are
// not objects (vpiMember fails).
//
// IEEE 1800-2009 37.16 (SystemVerilog-1800-2009.txt L58411-58470) draws
// struct, enum, integer, time, logic and packed array nets; detail 1
// (L58483-58486): "Any packed struct net or enum net declared with one or
// more explicit packed ranges is a packed array net. ... It shall not
// return the implicit range of packed struct net elements themselves, nor
// shall it return the range (explicit or implicit) for the base type of
// enum net elements." Detail 3 (L58502): "For logic nets, net bits shall
// be available regardless of vector expansion."
//
// Build A19-D48_typed_nets.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d48()` declared as a system task).
`timescale 1ns/1ns
typedef struct packed { logic [1:0] a; logic b; } st_t;
typedef enum logic [1:0] { A, B, C } en_t;
module tb;
  wire st_t sn;
  wire en_t en;
  wire integer inn;
  wire time tn;
  wire [1:0][3:0] pn;
  wire logic [3:0] ln;
  wire struct packed { logic x; logic [2:0] y; } an;
  wire st_t sarr [0:1];
  wire union packed { logic [3:0] u1; logic [3:0] u2; } un;
  assign sn = 3'b101;
  assign en = B;
  assign inn = -5;
  assign tn = 64'd7;
  assign pn = 8'h3c;
  assign ln = 4'h9;
  initial #1 $d48;
endmodule

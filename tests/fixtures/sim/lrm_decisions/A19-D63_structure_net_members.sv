// Decision A19-D63: the members of a structure or union net.
// IEEE 1800-2009 37.16 (L58454) draws vpiMember from a struct net; the text
// does not say which object type a member has. 37.16 detail 1 and Annex M
// give 1800-2009 no union net type.
// Decision: vpi_iterate(vpiMember) of a structure net, of a union net and
// of an element of an array of structure nets lists one net per member, in
// declaration order: a structure member is a vpiStructNet, an enum member a
// vpiEnumNet, any other member a vpiNet (a union member included, as a
// union net is a vpiNet). A member is named `<net>.<member>`, can be found
// by that name, has the enclosing member or net as vpiParent,
// vpiStructUnionMember TRUE, the net type of its net and the value of its
// bits of the net.
`timescale 1ns/1ns
module tb;
  typedef enum logic [1:0] { IDLE, RUN, STOP } st_t;
  typedef struct packed { logic [1:0] f; logic g; } s_t;
  typedef union packed { logic [2:0] a; s_t b; } u_t;
  typedef struct packed { st_t st; s_t in; } o_t;
  wire s_t sn;
  wire u_t un;
  wand o_t on;
  wire s_t sarr [0:1];
  assign sn = 3'b101;
  assign un = 3'b011;
  assign on = 5'b10110;
  assign sarr[1] = 3'b010;
  initial #1 $d63;
endmodule

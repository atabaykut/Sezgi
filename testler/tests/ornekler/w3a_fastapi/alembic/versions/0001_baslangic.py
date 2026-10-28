import sqlalchemy as sa
from alembic import op


def upgrade():
    op.create_table(
        "varlik",
        sa.Column("id", sa.Uuid(), nullable=False),
    )
    op.create_table(
        "etiket",
        sa.Column("id", sa.Uuid(), nullable=False),
        sa.Column("varlik_id", sa.Uuid(), nullable=False),
        sa.ForeignKeyConstraint(["varlik_id"], ["varlik.id"], ondelete="CASCADE"),
    )
    op.create_foreign_key(None, "yorum", "varlik", ["varlik_id"], ["id"], ondelete="CASCADE")
